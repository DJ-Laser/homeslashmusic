use std::{collections::HashSet, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
  Playing,
  Paused,
  Stopped,
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
  None,
  Track,
  Playlist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeekPosition {
  Forward(Duration),
  Backward(Duration),
  To(Duration),
}

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
  /// The cannonical file path
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
