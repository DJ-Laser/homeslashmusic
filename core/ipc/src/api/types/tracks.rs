use std::{collections::HashSet, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

/// Loaded data for a single audio track, typically one per file
/// Many `TrackId`s can be instances of the same `TrackMetadata`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrackMetadata {
  /// The cannonical, non-symlink file path
  pub file_path: PathBuf,
  pub total_duration: Option<Duration>,

  pub title: Option<String>,
  pub artists: HashSet<String>,
  pub album: Option<String>,
  pub track_number: Option<usize>,
  pub date: Option<String>,
  pub genres: HashSet<String>,
  pub comments: Vec<String>,
}

/// A unique id given to an instance of a `Track` in the track list
/// If there are multiple instances of the same track, each will have
/// a different `TrackId`
pub type TrackId = usize;

/// An instance of a track in the track list
/// The same track or file may have many instances, and each will have a unique id
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrackInstance {
  pub id: TrackId,
  pub metadata: TrackMetadata,
}
