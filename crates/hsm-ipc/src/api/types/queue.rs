use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::TrackId;

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
pub struct QueuedTrackId(u64);

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
#[non_exhaustive]
pub struct QueuedTrackMetadata {
    pub track_id: TrackId,
    pub total_duration: Option<Duration>,

    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub date: Option<String>,
    pub genres: Vec<String>,
}

impl QueuedTrackMetadata {
    /// Creates an empty `TrackMetadata` from a `QueuedTrackId`
    pub fn new(track_id: TrackId) -> Self {
        Self {
            track_id,
            total_duration: None,

            title: None,
            artist: None,
            album: None,
            track_number: None,
            date: None,
            genres: Vec::new(),
        }
    }
}
