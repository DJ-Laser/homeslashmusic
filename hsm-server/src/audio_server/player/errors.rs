use std::{io, path::PathBuf};

use rodio::decoder::DecoderError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlayerError {
  /// Should never happen since the player managers both ends of the channel
  #[error("Internal Player Error: SourceEvent channel closed")]
  SourceChannelClosed,

  /// Since we use an unbounded channel, an error means it must be closed
  #[error("Event channel closed")]
  EventChannelClosed,
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

#[derive(Debug, Error)]
pub enum SeekError {
  #[error("Internal Player Error: SeekError channel closed")]
  ErrorChannelClosed,

  #[error("Failed to seek: {0}")]
  SeekFailed(String),
}
