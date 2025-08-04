use std::io;

use symphonia::core::errors::Error as SymphoniaError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlayerError {
  /// Should never happen since the player managers both ends of the channel
  #[error("Internal Player Error: SourceEvent channel closed")]
  SourceChannelClosed,

  /// Since we use an unbounded channel, an error means it must be closed
  #[error("Event channel closed")]
  EventChannelClosed,

  #[error("Failed to load track: {0}")]
  LoadTrack(#[from] LoadTrackError),
}

impl PlayerError {
  pub fn is_recoverable(&self) -> bool {
    match self {
      PlayerError::LoadTrack(_) => true,
      _ => false,
    }
  }
}

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

#[derive(Debug, Error)]
pub enum SeekError {
  #[error("Internal Player Error: SeekError channel closed")]
  ErrorChannelClosed,

  #[error("{0}")]
  SeekFailed(String),
}
