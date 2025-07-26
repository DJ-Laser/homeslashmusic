use std::path::PathBuf;

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

  Volume { volume: f32 },

  Loop { loop_mode: LoopMode },
  SetTrack { path: PathBuf },
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
