use std::path;

use clap::Parser;
use cli::{Cli, Command};
use hsm_ipc::{InsertPosition, requests};
use ipc::send_request;

mod cli;
mod error;
mod ipc;

pub use error::Error;

fn handle_command(command: Cli) -> Result<(), crate::Error> {
  let reply: Result<(), String> = match command.command {
    Command::Play => send_request(requests::Playback::Play)?,
    Command::Pause => send_request(requests::Playback::Pause)?,
    Command::PlayPause => send_request(requests::Playback::Toggle)?,
    Command::Stop => send_request(requests::Playback::Stop)?,

    Command::Volume { volume } => send_request(requests::Set::Volume(volume))?,
    Command::Loop { loop_mode } => send_request(requests::Set::LoopMode(loop_mode.into()))?,
    Command::Seek { seek_position } => send_request(requests::Seek::new(seek_position.0))?,
    Command::SetTrack { path } => {
      let path = path::absolute(path).map_err(crate::Error::GetCurrentDirFailed)?;
      let res = send_request(requests::LoadTracks {
        paths: vec![path],
        position: InsertPosition::Relative(0),
      })?;

      res.map(|errors| {
        for (path, error) in errors {
          eprintln!("Failed to load track {path:?}: {error}")
        }
      })
    }
  };

  if let Err(message) = reply {
    return Err(crate::Error::Server(message));
  }

  Ok(())
}

fn main() -> Result<(), crate::Error> {
  let command = Cli::parse();

  handle_command(command)
}
