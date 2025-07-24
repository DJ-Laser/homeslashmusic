use std::{io, path::PathBuf};

use rodio::decoder::DecoderError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlayerError {
  #[error("Internal Player Error: SourceEvent channel closed")]
  SourceEventChannelClosed,
}

#[derive(Debug, Error)]
pub enum LoadTrackError {
  #[error("Could not load track: File {path} does not exist")]
  FileNotFound { path: PathBuf, source: io::Error },

  #[error("Could not load track: failed to get metadata for file {path}")]
  MetadataFailed { path: PathBuf, source: io::Error },

  #[error("Could not load track: {0}")]
  Decoder(#[from] DecoderError),
}
