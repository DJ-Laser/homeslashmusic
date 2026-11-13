mod albums;
mod artists;
mod mbid;
mod playlists;
mod songs;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;
use thiserror::Error;

pub use albums::*;
pub use artists::*;
pub use mbid::*;
pub use playlists::*;
pub use songs::*;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LookupMaps {
    song_by_mbid: HashMap<MusicBrainzId, SongId>,
    album_by_mbid: HashMap<MusicBrainzId, AlbumId>,
    artist_by_mbid: HashMap<MusicBrainzId, ArtistId>,
}

impl LookupMaps {
    fn update_song(&mut self, id: SongId, song: &SongData) {
        if let Some(mbid) = song.mbid {
            self.song_by_mbid.insert(mbid, id);
        }
    }

    fn update_album(&mut self, id: AlbumId, album: &AlbumData) {
        if let Some(mbid) = album.mbid {
            self.album_by_mbid.insert(mbid, id);
        }
    }

    fn update_artist(&mut self, id: ArtistId, artist: &ArtistData) {
        if let Some(mbid) = artist.mbid {
            self.artist_by_mbid.insert(mbid, id);
        }
    }

    fn update_playlist(&mut self, id: PlaylistId, playlist: &PlaylistData) {
        let _ = self;
        let _ = id;
        let _ = playlist;
    }
}

#[derive(Error, Debug)]
pub enum MissingDatabaseId {
    #[error("Database does not contain key {0:?}")]
    Song(SongId),
    #[error("Database does not contain key {0:?}")]
    Album(AlbumId),
    #[error("Database does not contain key {0:?}")]
    Artist(ArtistId),
    #[error("Database does not contain key {0:?}")]
    Playlist(PlaylistId),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DatabaseStore {
    songs: DenseSlotMap<SongId, SongData>,
    albums: DenseSlotMap<AlbumId, AlbumData>,
    artists: DenseSlotMap<ArtistId, ArtistData>,
    playlists: DenseSlotMap<PlaylistId, PlaylistData>,

    lookup: LookupMaps,
    unqueried_mbids: HashSet<MusicBrainzId>,
}

impl DatabaseStore {
    /// Add a query for a `MusicBrainzId` to the pending set
    /// Returns the mbid if a query is already pending
    pub fn claim_mbid_query(&mut self, mbid: MusicBrainzId) -> Result<(), MusicBrainzId> {
        if self.unqueried_mbids.contains(&mbid) {
            return Err(mbid);
        }

        self.unqueried_mbids.insert(mbid);
        Ok(())
    }

    /// Mark the query for the specified mbid as completed and remove it from the pending set
    pub fn complete_mbid_query(&mut self, mbid: &MusicBrainzId) {
        self.unqueried_mbids.remove(mbid);
    }
}
