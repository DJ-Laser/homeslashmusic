use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::TrackId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version(pub String);

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
pub enum SeekOffset {
  Forward(Duration),
  Backward(Duration),
}

impl TryFrom<SeekPosition> for SeekOffset {
  type Error = Duration;

  fn try_from(value: SeekPosition) -> Result<Self, Self::Error> {
    match value {
      SeekPosition::Forward(d) => Ok(Self::Forward(d)),
      SeekPosition::Backward(d) => Ok(Self::Backward(d)),
      SeekPosition::To(d) => Err(d),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeekPosition {
  Forward(Duration),
  Backward(Duration),
  To(Duration),
}

impl From<SeekOffset> for SeekPosition {
  fn from(value: SeekOffset) -> Self {
    match value {
      SeekOffset::Forward(d) => Self::Forward(d),
      SeekOffset::Backward(d) => Self::Backward(d),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InsertPosition {
  After(TrackId),
  Next,
  Start,
  End,
}
