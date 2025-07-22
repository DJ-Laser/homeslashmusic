use std::path::PathBuf;

pub enum PlaybackControl {
  Play,
  Pause,
  Toggle,
}

pub enum Message {
  Playback(PlaybackControl),
  SetTrack(PathBuf),
}
