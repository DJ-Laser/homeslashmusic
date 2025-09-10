use std::{path::PathBuf, time::Duration};

use async_oneshot as oneshot;
use hsm_ipc::{InsertPosition, LoopMode, PlaybackState, SeekPosition, Track, TrackListSnapshot};

use super::track::LoadTrackError;

pub enum Query {
  PlaybackState(oneshot::Sender<PlaybackState>),
  LoopMode(oneshot::Sender<LoopMode>),
  Volume(oneshot::Sender<f32>),
  Shuffle(oneshot::Sender<bool>),
  Position(oneshot::Sender<Duration>),
  CurrentTrack(oneshot::Sender<Option<Track>>),
  CurrentTrackIndex(oneshot::Sender<usize>),
  IpcTrackList(oneshot::Sender<TrackListSnapshot>),
}

pub enum Message {
  Play,
  Pause,
  Toggle,
  Stop,
  NextTrack,
  PreviousTrack {
    /// Restarts the track instead of going to the previous track if enough time has passed
    soft: bool,
  },
  SetLoopMode(LoopMode),
  SetVolume(f32),
  SetShuffle(bool),
  Seek(SeekPosition),
  InsertTracks {
    paths: Vec<PathBuf>,
    position: InsertPosition,
    error_tx: oneshot::Sender<Vec<(PathBuf, LoadTrackError)>>,
  },
  ClearTracks,
  Query(Query),
}
