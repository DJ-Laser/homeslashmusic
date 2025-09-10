use std::{collections::HashSet, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrackMetadata {
  pub title: Option<String>,
  pub artists: HashSet<String>,
  pub album: Option<String>,
  pub track_number: Option<usize>,
  pub date: Option<String>,
  pub genres: HashSet<String>,
  pub comments: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
  /// The cannonical, non-symlink file path
  pub file_path: PathBuf,
  pub total_duration: Option<Duration>,
  pub metadata: TrackMetadata,
}

/// A representation of the player's track list
/// `track_list.len()` will always be equal to `shuffle_indicies.len()`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackListSnapshot {
  pub track_list: Vec<Track>,
  pub shuffle_indicies: Vec<usize>,
}

pub enum TrackListUpdate {
  Insert {
    index: usize,
    tracks: Vec<Track>,
    new_shuffle_indicies: Vec<usize>,
  },

  Remove {
    removed_indicies: Vec<usize>,
    new_shuffle_indicies: Vec<usize>,
  },

  Replace(TrackListSnapshot),

  Clear,

  Shuffle {
    new_shuffle_indicies: Vec<usize>,
  },
}
