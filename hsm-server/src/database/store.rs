mod albums;
mod artists;
mod mbid;
mod playlists;
mod songs;
mod tracks;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;
use thiserror::Error;

pub use albums::*;
pub use artists::*;
pub use mbid::*;
pub use playlists::*;
pub use songs::*;
pub use tracks::*;

#[derive(Error, Debug)]
pub enum MissingDatabaseId {
    #[error("Database does not contain key {0:?}")]
    Song(SongId),
    #[error("Database does not contain key {0:?}")]
    Track(TrackId),
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
    tracks: DenseSlotMap<TrackId, TrackData>,
    albums: DenseSlotMap<AlbumId, AlbumData>,
    artists: DenseSlotMap<ArtistId, ArtistData>,
    playlists: DenseSlotMap<PlaylistId, PlaylistData>,

    song_by_mbid: HashMap<MusicBrainzId, SongId>,
    album_by_mbid: HashMap<MusicBrainzId, AlbumId>,
    artist_by_mbid: HashMap<MusicBrainzId, ArtistId>,
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
