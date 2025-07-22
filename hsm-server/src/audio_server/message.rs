pub enum PlaybackControl {
  Play,
  Pause,
  Toggle,
}

pub enum Message {
  Playback(PlaybackControl),
}
