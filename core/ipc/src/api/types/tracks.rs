use std::{collections::HashSet, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioSpec {
  pub track_id: u32,
  pub bit_depth: Option<u32>,
  pub channel_bitmask: u32,
  pub channel_count: u16,
  pub sample_rate: u32,
  pub total_duration: Option<Duration>,
}

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
  file_path: PathBuf,
  audio_spec: AudioSpec,
  metadata: TrackMetadata,
}

impl Track {
  pub fn new(file_path: PathBuf, audio_spec: AudioSpec, metadata: TrackMetadata) -> Self {
    Self {
      file_path,
      audio_spec,
      metadata,
    }
  }

  pub fn file_path(&self) -> &PathBuf {
    &self.file_path
  }

  pub fn audio_spec(&self) -> &AudioSpec {
    &self.audio_spec
  }

  pub fn metadata(&self) -> &TrackMetadata {
    &self.metadata
  }
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
