use std::time::Duration;

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
