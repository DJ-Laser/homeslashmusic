use std::{
  io,
  path::{Path, PathBuf},
};

pub use cache::TrackCache;
pub use loading::{load_file, probe_track_sync};
use smol::fs;
use symphonia::core::errors::Error as SymphoniaError;
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

pub async fn get_cannonical_track_path(path: &Path) -> Result<PathBuf, LoadTrackError> {
  fs::canonicalize(&path)
    .await
    .map_err(|error| LoadTrackError::CannonicalizeFailed(error))
}
