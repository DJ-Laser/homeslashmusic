use std::time::Duration;

use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;

use crate::database::store::{
    ArtistData, ArtistId, DatabaseStore, MissingDatabaseId, MusicBrainzId, ResourceLocation,
    TrackId,
};

slotmap::new_key_type! {
    pub struct SongId;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DerivedSongData {
    pub(super) tracks: Vec<TrackId>,
}

impl DerivedSongData {
    /// The tracks that contain audio for this song
    pub fn tracks(&self) -> &Vec<TrackId> {
        &self.tracks
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SongData {
    /// The `MusicBrainzId` for this song
    pub mbid: Option<MusicBrainzId>,
    /// Song data derrived other database entries
    pub derived: DerivedSongData,

    /// Name of the song
    pub title: Option<String>,
    /// Cover art for the song
    pub image: Option<ResourceLocation>,
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

    /// Contains the songs included in this one if it is a mix or dj set
    pub mix_songs: Vec<SongId>,
}

impl DatabaseStore {
    pub fn get_song(&self, id: SongId) -> Result<&SongData, MissingDatabaseId> {
        self.songs.get(id).ok_or(MissingDatabaseId::Song(id))
    }

    pub fn lookup_song_by_mbid(&self, mbid: &MusicBrainzId) -> Option<SongId> {
        self.song_by_mbid.get(mbid).copied()
    }

    fn update_song_artists(
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
                artist.derived.songs.retain(|stored_id| id != *stored_id);
            } else if !artist.derived.songs.contains(&id) {
                artist.derived.songs.push(id);
            }
        }
    }

    pub fn insert_song(&mut self, song: SongData) -> SongId {
        self.songs.insert_with_key(|id| {
            if let Some(mbid) = song.mbid {
                self.song_by_mbid.insert(mbid, id);
            }

            Self::update_song_artists(&mut self.artists, id, &song, false);

            song
        })
    }

    pub fn update_or_insert_song_by_mbid(
        &mut self,
        mbid: MusicBrainzId,
        mut song: SongData,
    ) -> Result<SongId, MissingDatabaseId> {
        let Some(id) = self.lookup_song_by_mbid(&mbid) else {
            song.mbid = Some(mbid);
            return Ok(self.insert_song(song));
        };

        let db_song = self.songs.get_mut(id).ok_or(MissingDatabaseId::Song(id))?;

        Self::update_song_artists(&mut self.artists, id, db_song, true);
        Self::update_song_artists(&mut self.artists, id, &song, false);

        db_song.title = song.title;
        db_song.image = song.image;
        db_song.artists = song.artists;
        db_song.artists_text = song.artists_text;
        db_song.genres = song.genres;
        db_song.date = song.date;
        db_song.length = song.length;
        db_song.mix_songs = song.mix_songs;

        Ok(id)
    }
}
