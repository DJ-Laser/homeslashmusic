use std::{fmt::Display, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{AlbumId, PlaylistId, SongId, TrackGroupId, TrackId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InsertItem {
    /// Insert the specified track
    Track(TrackId),
    /// Insert the preferred track for the song
    Song(SongId),
    /// Insert the specified track group
    TrackGroup(TrackGroupId),
    /// Insert the preferred track group for the album
    Album(AlbumId),
    // /// Insert the contents of the playlist
    Playlist(PlaylistId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InsertPosition {
    After(QueuedTrackId),
    Next,
    Start,
    End,
}

/// A unique id given to an instance of a `Track` in the queue
/// If there are multiple instances of the same track, each will have a different `QueuedTrackId`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedTrackId(pub usize);

impl Display for QueuedTrackId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// An instance of a track in the queue
/// The same track may have many instances, and each will have a unique id
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueuedTrack {
    pub id: QueuedTrackId,
    #[serde(flatten)]
    pub metadata: QueuedTrackMetadata,
}

/// A subset of a database track's metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueuedTrackMetadata {
    pub track_id: TrackId,
    pub length: Duration,

    pub title: String,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub artist_text: Option<String>,
    pub date: Option<String>,
    pub genres: Vec<String>,
}
