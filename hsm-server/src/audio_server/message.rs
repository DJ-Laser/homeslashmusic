use std::{path::PathBuf, sync::Arc, time::Duration};

use async_oneshot as oneshot;
use hsm_ipc::{InsertPosition, LoopMode, PlaybackState, SeekPosition, Track};

use super::player::errors::LoadTrackError;

pub enum Query {
  PlaybackState(oneshot::Sender<PlaybackState>),
  LoopMode(oneshot::Sender<LoopMode>),
  Volume(oneshot::Sender<f32>),
  Position(oneshot::Sender<Duration>),
  CurrentTrack(oneshot::Sender<Option<Arc<Track>>>),
}

pub enum Message {
  Play,
  Pause,
  Toggle,
  Stop,
  SetLoopMode(LoopMode),
  SetVolume(f32),
  Seek(SeekPosition),
  InsertTracks {
    paths: Vec<PathBuf>,
    position: InsertPosition,
    error_tx: oneshot::Sender<Vec<(PathBuf, LoadTrackError)>>,
  },
  Query(Query),
}
