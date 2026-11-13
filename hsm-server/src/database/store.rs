mod albums;
mod artists;
mod mbid;
mod playlists;
mod tracks;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;
use thiserror::Error;

pub use albums::*;
pub use artists::*;
pub use mbid::*;
pub use playlists::*;
pub use tracks::*;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LookupMaps {
    track_by_audio_location: HashMap<ResourceLocation, TrackId>,
    track_by_mbid: HashMap<MusicBrainzId, TrackId>,
    album_by_mbid: HashMap<MusicBrainzId, AlbumId>,
    artist_by_mbid: HashMap<MusicBrainzId, ArtistId>,
}

impl LookupMaps {
    fn update_track(&mut self, id: TrackId, track: &TrackData) {
        if let Some(mbid) = track.mbid {
            self.track_by_mbid.insert(mbid, id);
        }

        for location in &track.audio_locations {
            self.track_by_audio_location.insert(location.clone(), id);
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
        let _ = id;
        let _ = playlist;
    }
}

#[derive(Error, Debug)]
pub enum MissingDatabaseId {
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
    tracks: DenseSlotMap<TrackId, TrackData>,
    albums: DenseSlotMap<AlbumId, AlbumData>,
    artists: DenseSlotMap<ArtistId, ArtistData>,
    playlists: DenseSlotMap<PlaylistId, PlaylistData>,

    lookup: LookupMaps,
    unqueried_mbids: HashSet<MusicBrainzId>,
}

impl DatabaseStore {
    /// Check if the database has a pending query for a certain `MusicBrainzId`
    /// If not, the mbid is added to the set of pending queries
    pub fn has_pending_mbid_query(&mut self, mbid: MusicBrainzId) -> bool {
        if self.unqueried_mbids.contains(&mbid) {
            return true;
        }

        self.unqueried_mbids.insert(mbid);
        false
    }

    /// Mark the query for the specified mbid as completed and remove it from the pending set
    pub fn complete_mbid_query(&mut self, mbid: &MusicBrainzId) {
        self.unqueried_mbids.remove(mbid);
    }
}
