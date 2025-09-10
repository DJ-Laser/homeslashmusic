use std::{
  io,
  path::{Path, PathBuf},
};

pub use cache::TrackCache;
use hsm_ipc::{Track, TrackMetadata};
pub use loading::{load_file, probe_track_sync};
use smol::fs;
use symphonia::core::{audio::SignalSpec, errors::Error as SymphoniaError};
use thiserror::Error;

mod cache;
mod loading;

#[derive(Debug, Error)]
pub enum LoadTrackError {
  #[error("{0}")]
  CannonicalizeFailed(#[source] io::Error),

  #[error("{0}")]
  OpenFailed(#[source] io::Error),

  #[error("{0}")]
  ReadDirFailed(#[source] io::Error),

  #[error("{0}")]
  ProbeFailed(#[source] SymphoniaError),

  #[error("Track has no supported audio codec")]
  CodecNotSupported,

  #[error("{0}")]
  DecodingFailed(#[source] SymphoniaError),
}

/// A `Track` that has been loaded into the cache
#[derive(Debug)]
pub struct LoadedTrack {
  pub inner: Track,
  pub spec: SignalSpec,
}

impl LoadedTrack {
  pub fn file_path(&self) -> &Path {
    &self.inner.file_path
  }

  pub fn metadata(&self) -> &TrackMetadata {
    &self.inner.metadata
  }

  pub fn clone_track(&self) -> Track {
    self.inner.clone()
  }
}

impl Into<Track> for LoadedTrack {
  fn into(self) -> Track {
    self.inner
  }
}

pub async fn get_cannonical_track_path(path: &Path) -> Result<PathBuf, LoadTrackError> {
  fs::canonicalize(&path)
    .await
    .map_err(|error| LoadTrackError::CannonicalizeFailed(error))
}
