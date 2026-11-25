use std::{fmt::Display, mem};

use log::error;
use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;

use crate::database::store::{
    ArtistData, ArtistId, DatabaseStore, Lookup, MissingDatabaseId, MusicBrainzId, PlaylistId,
    TrackGroupId,
};

slotmap::new_key_type! {
    pub struct AlbumId;
}

impl Display for AlbumId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DerivedAlbumData {
    pub(super) track_groups: Vec<TrackGroupId>,
    pub(super) in_playlists: Vec<PlaylistId>,
}

impl DerivedAlbumData {
    /// Track groups for this album
    pub fn track_groups(&self) -> &Vec<TrackGroupId> {
        &self.track_groups
    }

    /// Playlists this album is in
    pub fn in_playlists(&self) -> &Vec<PlaylistId> {
        &self.in_playlists
    }

    pub(super) fn clear(&mut self) {
        self.track_groups.clear();
        self.in_playlists.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumData {
    /// The `MusicBrainzId` for this album
    pub mbid: MusicBrainzId,
    /// Album data derrived other database entries
    #[serde(skip)]
    pub derived: DerivedAlbumData,

    /// Name of the album
    pub title: String,
    /// The date this album was released
    pub date: Option<String>,
    /// The genres associated with this album, sorted by relevance
    pub genres: Vec<String>,

    /// Artists assocated with this album, sorted as they appear on the album metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the album's artists. May include separators like "feat."
    pub artists_text: Option<String>,
}

impl AlbumData {
    pub fn search_string(&self) -> String {
        if let Some(artists_text) = &self.artists_text {
            format!("{} - {}", self.title, artists_text)
        } else {
            self.title.clone()
        }
    }
}

impl DatabaseStore {
    pub fn get_album(&self, id: AlbumId) -> Result<&AlbumData, MissingDatabaseId> {
        self.albums.get(id).ok_or(MissingDatabaseId::Album(id))
    }

    pub fn lookup_album(&self, mbid: MusicBrainzId) -> Option<AlbumId> {
        self.lookup.album.get(&mbid).copied()
    }

    fn insert_album(&mut self, album: AlbumData) -> AlbumId {
        self.albums.insert_with_key(|id| {
            self.lookup.insert_album(album.mbid, id);

            Self::update_album_links(&mut self.artists, id, &album, false);

            album
        })
    }

    pub(super) fn update_album_links(
        artists: &mut DenseSlotMap<ArtistId, ArtistData>,
        id: AlbumId,
        album: &AlbumData,
        remove: bool,
    ) {
        for artist_id in &album.artists {
            let Some(artist) = artists.get_mut(*artist_id) else {
                continue;
            };

            if remove {
                artist.derived.albums.retain(|stored_id| *stored_id != id);
            } else if !artist.derived.albums.contains(&id) {
                artist.derived.albums.push(id);
            }
        }
    }

    pub fn update_or_insert_album(&mut self, album: AlbumData) -> AlbumId {
        let Some(id) = self.lookup_album(album.mbid) else {
            return self.insert_album(album);
        };

        let Some(db_album) = self.albums.get_mut(id) else {
            error!(
                "Error updating album with mbid {}: {}",
                album.mbid,
                MissingDatabaseId::Album(id)
            );

            return self.insert_album(album);
        };

        Self::update_album_links(&mut self.artists, id, db_album, true);

        let old_album = mem::replace(db_album, album);
        db_album.derived = old_album.derived;

        Self::update_album_links(&mut self.artists, id, db_album, false);

        id
    }
}

impl Lookup {
    pub fn insert_album(&mut self, mbid: MusicBrainzId, id: AlbumId) {
        self.album.insert(mbid, id);
    }
}
