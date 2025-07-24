use clap::Parser;
use cli::{Cli, Command};
use hsm_ipc::requests;
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

    Command::SetVolume { volume } => send_request(requests::Set::Volume(volume))?,
    Command::SetLoop { loop_mode } => send_request(requests::Set::LoopMode(loop_mode.into()))?,
    Command::SetTrack { path } => send_request(requests::LoadTrack::new(path))?,
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
