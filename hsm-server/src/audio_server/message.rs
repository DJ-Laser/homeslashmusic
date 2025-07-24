use std::path::PathBuf;

use async_oneshot as oneshot;

use super::player::{LoopMode, PlaybackState};

pub enum Query {
  PlaybackState(oneshot::Sender<PlaybackState>),
  LoopMode(oneshot::Sender<LoopMode>),
  Volume(oneshot::Sender<f32>),
}

pub enum Message {
  Play,
  Pause,
  Toggle,
  Stop,
  SetLoopMode(LoopMode),
  SetVolume(f32),
  SetTrack(PathBuf),
  Query(Query),
}
