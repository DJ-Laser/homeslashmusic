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
}

#[derive(Debug, Error)]
pub enum LoadTrackError {
  #[error("Could not load track: {0}")]
  CannonicalizeFailed(#[source] io::Error),

  #[error("Could not load track: {0}")]
  OpenFailed(#[source] io::Error),

  #[error("Could not load track: {0}")]
  ProbeFailed(#[source] SymphoniaError),

  #[error("Could not load track: Track has no supported audio codec")]
  CodecNotSupported,

  #[error("Could not load track: {0}")]
  DecodingFailed(#[source] SymphoniaError),
}

#[derive(Debug, Error)]
pub enum SeekError {
  #[error("Internal Player Error: SeekError channel closed")]
  ErrorChannelClosed,

  #[error("Failed to seek: {0}")]
  SeekFailed(String),
}
