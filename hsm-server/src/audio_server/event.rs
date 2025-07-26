use std::time::Duration;

use hsm_ipc::{LoopMode, PlaybackState};

#[derive(Debug, Clone)]
pub enum Event {
  PlaybackStateChanged(PlaybackState),
  LoopModeChanged(LoopMode),
  VolumeChanged(f32),
  Seeked(Duration),
}
