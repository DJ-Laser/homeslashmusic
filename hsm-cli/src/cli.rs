use std::{num::ParseFloatError, path::PathBuf, str::FromStr, time::Duration};

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
pub struct Cli {
  #[command(subcommand)]
  pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
  Play,
  Pause,
  PlayPause,
  Stop,

  Volume {
    volume: f32,
  },
  Loop {
    loop_mode: LoopMode,
  },
  Seek {
    #[arg(allow_negative_numbers = true)]
    seek_position: SeekPosition,
  },
  SetTrack {
    path: PathBuf,
  },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum LoopMode {
  Off,
  #[value(alias = "on")]
  Track,
  Playlist,
}

impl Into<hsm_ipc::LoopMode> for LoopMode {
  fn into(self) -> hsm_ipc::LoopMode {
    match self {
      LoopMode::Off => hsm_ipc::LoopMode::None,
      LoopMode::Track => hsm_ipc::LoopMode::Track,
      LoopMode::Playlist => hsm_ipc::LoopMode::Playlist,
    }
  }
}

#[derive(Debug, Clone)]
pub struct SeekPosition(pub hsm_ipc::SeekPosition);

impl FromStr for SeekPosition {
  type Err = ParseFloatError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if let Some(s) = s.strip_prefix("+") {
      let secs: f64 = s.parse()?;
      return Ok(Self(hsm_ipc::SeekPosition::Forward(
        Duration::from_secs_f64(secs),
      )));
    }

    if let Some(s) = s.strip_prefix("-") {
      let secs: f64 = s.parse()?;
      return Ok(Self(hsm_ipc::SeekPosition::Backward(
        Duration::from_secs_f64(secs),
      )));
    }

    let secs: f64 = s.parse()?;
    Ok(Self(hsm_ipc::SeekPosition::To(Duration::from_secs_f64(
      secs,
    ))))
  }
}
