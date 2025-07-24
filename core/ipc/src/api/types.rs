use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlaybackState {
  Playing,
  Paused,
  Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoopMode {
  None,
  Track,
  Playlist,
}
