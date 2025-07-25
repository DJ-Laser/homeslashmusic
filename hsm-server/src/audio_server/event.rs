use hsm_ipc::{LoopMode, PlaybackState};

#[derive(Debug, Clone)]
pub enum Event {
  PlaybackStateChanged(PlaybackState),
  LoopModeChanged(LoopMode),
  VolumeChanged(f32),
}
