use std::{path::PathBuf, time::Duration};

use async_oneshot as oneshot;
use hsm_ipc::{LoopMode, PlaybackState, SeekPosition};

pub enum Query {
  PlaybackState(oneshot::Sender<PlaybackState>),
  LoopMode(oneshot::Sender<LoopMode>),
  Volume(oneshot::Sender<f32>),
  Position(oneshot::Sender<Duration>),
}

pub enum Message {
  Play,
  Pause,
  Toggle,
  Stop,
  SetLoopMode(LoopMode),
  SetVolume(f32),
  Seek(SeekPosition),
  SetTrack(PathBuf),
  Query(Query),
}
