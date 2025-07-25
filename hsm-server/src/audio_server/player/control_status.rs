use atomic_enum::atomic_enum;

#[atomic_enum]
#[derive(PartialEq, Eq)]
pub enum PlaybackState {
  Playing,
  Paused,
  Stopped,
}

impl From<hsm_ipc::PlaybackState> for PlaybackState {
  fn from(value: hsm_ipc::PlaybackState) -> Self {
    match value {
      hsm_ipc::PlaybackState::Playing => PlaybackState::Playing,
      hsm_ipc::PlaybackState::Paused => PlaybackState::Paused,
      hsm_ipc::PlaybackState::Stopped => PlaybackState::Stopped,
    }
  }
}

#[atomic_enum]
#[derive(PartialEq, Eq)]
pub enum LoopMode {
  None,
  Track,
  Playlist,
}

impl From<hsm_ipc::LoopMode> for LoopMode {
  fn from(value: hsm_ipc::LoopMode) -> Self {
    match value {
      hsm_ipc::LoopMode::None => LoopMode::None,
      hsm_ipc::LoopMode::Track => LoopMode::Track,
      hsm_ipc::LoopMode::Playlist => LoopMode::Playlist,
    }
  }
}
