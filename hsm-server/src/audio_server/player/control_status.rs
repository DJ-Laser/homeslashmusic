use atomic_enum::atomic_enum;

#[atomic_enum]
pub enum PlaybackState {
  Playing,
  Paused,
  Stopped,
}

#[atomic_enum]
pub enum LoopMode {
  None,
  Track,
  Playlist,
}
