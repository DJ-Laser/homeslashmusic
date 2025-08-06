use std::{num::ParseFloatError, path::PathBuf, time::Duration};

use clap::{Args, Parser, Subcommand, ValueEnum};
use hsm_ipc::SeekPosition;

#[derive(Debug, Parser)]
pub struct Cli {
  #[command(subcommand)]
  pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
  Play {
    #[command(flatten)]
    tracks: Option<TrackPaths>,
  },

  Pause,
  PlayPause,
  Stop,

  Volume {
    volume: f32,
  },
  Loop {
    loop_mode: LoopMode,
  },
  Shuffle {
    shuffle: ShuffleMode,
  },

  Seek {
    #[arg(value_parser = parse_seek_position)]
    #[arg(allow_negative_numbers = true)]
    seek_position: SeekPosition,
  },

  #[command(args_conflicts_with_subcommands = true)]
  Queue {
    #[command(subcommand)]
    command: Option<QueueCommand>,
    #[command(flatten)]
    tracks: Option<TrackPaths>,
  },
}

#[derive(Debug, Subcommand)]
pub enum QueueCommand {
  Clear,
  Replace {
    #[command(flatten)]
    tracks: TrackPaths,
  },
  #[command(alias = "append")]
  Add {
    #[command(flatten)]
    tracks: TrackPaths,
  },
  Next {
    #[command(flatten)]
    tracks: TrackPaths,
  },
}

#[derive(Debug, Args)]
pub struct TrackPaths {
  #[arg(num_args = 1..)]
  pub paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum LoopMode {
  Off,
  #[value(alias = "one")]
  Track,
  #[value(aliases = ["on", "all"])]
  Playlist,
}

impl Into<hsm_ipc::LoopMode> for LoopMode {
  fn into(self) -> hsm_ipc::LoopMode {
    match self {
      Self::Off => hsm_ipc::LoopMode::None,
      Self::Track => hsm_ipc::LoopMode::Track,
      Self::Playlist => hsm_ipc::LoopMode::Playlist,
    }
  }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ShuffleMode {
  Off,
  On,
}

impl Into<bool> for ShuffleMode {
  fn into(self) -> bool {
    match self {
      Self::Off => false,
      Self::On => true,
    }
  }
}

fn parse_seek_position(s: &str) -> Result<SeekPosition, ParseFloatError> {
  if let Some(s) = s.strip_prefix("+") {
    let secs: f64 = s.parse()?;
    return Ok(SeekPosition::Forward(Duration::from_secs_f64(secs)));
  }

  if let Some(s) = s.strip_prefix("-") {
    let secs: f64 = s.parse()?;
    return Ok(SeekPosition::Backward(Duration::from_secs_f64(secs)));
  }

  let secs: f64 = s.parse()?;
  Ok(SeekPosition::To(Duration::from_secs_f64(secs)))
}
