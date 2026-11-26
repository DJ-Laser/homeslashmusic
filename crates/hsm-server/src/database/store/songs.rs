use std::{collections::HashMap, fmt::Display, mem, time::Duration};

use log::error;
use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;

use crate::database::store::{
    ArtistData, ArtistId, DatabaseStore, Lookup, MissingDatabaseId, MusicBrainzId, PlaylistId,
    TrackId,
};

slotmap::new_key_type! {
    pub struct SongId;
}

impl Display for SongId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SongRelationship {
    Remix,
    Mashup,
    DjMix,
    Compilation,
}

impl Display for SongRelationship {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SongRelationship::Remix => f.write_str("Remix"),
            SongRelationship::Mashup => f.write_str("Mashup"),
            SongRelationship::DjMix => f.write_str("DJ-Mix"),
            SongRelationship::Compilation => f.write_str("Compilation"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DerivedSongData {
    pub(super) tracks: Vec<TrackId>,
    pub(super) related_to: HashMap<SongRelationship, Vec<SongId>>,
    pub(super) in_playlists: Vec<PlaylistId>,
}

impl DerivedSongData {
    /// The tracks that contain audio for this song
    pub fn tracks(&self) -> &Vec<TrackId> {
        &self.tracks
    }

    /// Songs that are remixes/compilations of this song
    pub fn related_to(&self) -> &HashMap<SongRelationship, Vec<SongId>> {
        &self.related_to
    }

    /// Playlists this song is in
    pub fn in_playlists(&self) -> &Vec<PlaylistId> {
        &self.in_playlists
    }

    pub(super) fn clear(&mut self) {
        self.tracks.clear();
        self.related_to.clear();
        self.in_playlists.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongData {
    /// The `MusicBrainzId` for this song
    pub mbid: MusicBrainzId,
    /// Song data derrived other database entries
    #[serde(skip)]
    pub derived: DerivedSongData,

    /// Name of the song
    pub title: String,
    /// The length of this song
    pub length: Option<Duration>,
    /// The date this song was released
    pub date: Option<String>,

    /// Artists associated with this song, sorted as they appear on the song metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the song's artists. May include separators like "feat."
    pub artists_text: Option<String>,

    /// The genres associated with this song, sorted by relevance
    pub genres: Vec<String>,

    /// Relationships from this song to the songs it remixes/compiles
    pub relationships: HashMap<SongRelationship, Vec<SongId>>,
}

impl DatabaseStore {
    pub fn get_song(&self, id: SongId) -> Result<&SongData, MissingDatabaseId> {
        self.songs.get(id).ok_or(MissingDatabaseId::Song(id))
    }

    pub fn lookup_song(&self, mbid: &MusicBrainzId) -> Option<SongId> {
        self.lookup.song.get(mbid).copied()
    }

    pub(super) fn update_song_links(
        artists: &mut DenseSlotMap<ArtistId, ArtistData>,
        id: SongId,
        song: &SongData,
        remove: bool,
    ) {
        for artist_id in &song.artists {
            let Some(artist) = artists.get_mut(*artist_id) else {
                continue;
            };

            if remove {
                artist.derived.songs.retain(|stored_id| *stored_id != id);
            } else if !artist.derived.songs.contains(&id) {
                artist.derived.songs.push(id);
            }
        }
    }

    pub fn update_song_relationship_links(
        songs: &mut DenseSlotMap<SongId, SongData>,
        id: SongId,
        remove: bool,
    ) {
        let Some(relationships) = songs
            .get_mut(id)
            .map(|song| mem::take(&mut song.relationships))
        else {
            return;
        };

        for (relationship, targets) in &relationships {
            for &target_id in targets {
                let Some(target) = songs.get_mut(target_id) else {
                    continue;
                };

                if remove {
                    if let Some(list) = target.derived.related_to.get_mut(relationship) {
                        list.retain(|song_id| *song_id != id);
                        if list.is_empty() {
                            target.derived.related_to.remove(relationship);
                        }
                    }
                } else {
                    let list = target
                        .derived
                        .related_to
                        .entry(relationship.clone())
                        .or_default();
                    if !list.contains(&id) {
                        list.push(id);
                    }
                }
            }
        }

        let Some(song) = songs.get_mut(id) else {
            return;
        };

        song.relationships = relationships;
    }

    fn fix_song_relationships(id: SongId, mut song: SongData) -> SongData {
        for targets in song.relationships.values_mut() {
            targets.retain(|song_id| *song_id != id);
        }

        song.relationships.retain(|_, targets| !targets.is_empty());

        song
    }

    fn insert_song(&mut self, song: SongData) -> SongId {
        let id = self.songs.insert_with_key(|id| {
            let song = Self::fix_song_relationships(id, song);
            self.lookup.insert_song(song.mbid, id);

            Self::update_song_links(&mut self.artists, id, &song, false);

            song
        });

        Self::update_song_relationship_links(&mut self.songs, id, false);

        id
    }

    pub fn update_or_insert_song(&mut self, song: SongData) -> SongId {
        let Some(id) = self.lookup_song(&song.mbid) else {
            return self.insert_song(song);
        };

        Self::update_song_relationship_links(&mut self.songs, id, true);

        let song = Self::fix_song_relationships(id, song);
        let Some(db_song) = self.songs.get_mut(id) else {
            error!(
                "Error updating song with mbid {}: {}",
                song.mbid,
                MissingDatabaseId::Song(id)
            );

            return self.insert_song(song);
        };

        Self::update_song_links(&mut self.artists, id, db_song, true);
        Self::update_song_links(&mut self.artists, id, &song, false);

        let old_song = mem::replace(db_song, song);
        db_song.derived = old_song.derived;

        Self::update_song_relationship_links(&mut self.songs, id, false);

        id
    }

    pub fn remove_song(&mut self, id: SongId) {
        Self::update_song_relationship_links(&mut self.songs, id, true);

        if let Some(song) = self.songs.remove(id) {
            self.lookup.song.remove(&song.mbid);

            Self::update_song_links(&mut self.artists, id, &song, true);
        }
    }
}

impl Lookup {
    pub fn insert_song(&mut self, mbid: MusicBrainzId, id: SongId) {
        self.song.insert(mbid, id);
    }
}
