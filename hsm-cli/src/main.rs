use std::path::{self, PathBuf};

use clap::Parser;
use cli::{Cli, Command, QueueCommand};
use hsm_ipc::{InsertPosition, requests};
use ipc::send_request;

mod cli;
mod error;
mod ipc;

pub use error::Error;

type StandardReply = Result<(), String>;

fn try_load_tracks(
  position: InsertPosition,
  paths: &[PathBuf],
) -> Result<StandardReply, crate::Error> {
  let mut absolute_paths = Vec::new();
  for path in paths {
    absolute_paths.push(path::absolute(path).map_err(crate::Error::GetCurrentDirFailed)?);
  }

  let res = send_request(requests::LoadTracks(position, absolute_paths))?;

  Ok(res.map(|errors| {
    for (path, error) in errors {
      eprintln!("Failed to load track {path:?}: {error}")
    }
  }))
}

fn handle_queue_command(command: QueueCommand) -> Result<StandardReply, crate::Error> {
  let res = match command {
    QueueCommand::Clear => send_request(requests::ClearTracks)?,
    QueueCommand::Replace { tracks } => try_load_tracks(InsertPosition::Replace, &tracks.paths)?,
    QueueCommand::Add { tracks } => try_load_tracks(InsertPosition::End, &tracks.paths)?,
    QueueCommand::Next { tracks } => try_load_tracks(InsertPosition::Next, &tracks.paths)?,
  };

  Ok(res)
}

fn handle_command(command: Cli) -> Result<(), crate::Error> {
  let reply: Result<(), String> = match command.command {
    Command::Play { tracks } => {
      let res = if let Some(tracks) = tracks {
        try_load_tracks(InsertPosition::Replace, &tracks.paths)?
      } else {
        Ok(())
      };

      match res {
        Ok(()) => send_request(requests::Play)?,
        Err(error) => Err(error),
      }
    }
    Command::Pause => send_request(requests::Pause)?,
    Command::PlayPause => send_request(requests::TogglePlayback)?,
    Command::Stop => send_request(requests::StopPlayback)?,

    Command::Next => send_request(requests::NextTrack)?,
    Command::Previous => send_request(requests::PreviousTrack { soft: true })?,

    Command::Loop { loop_mode } => send_request(requests::SetLoopMode(loop_mode.into()))?,
    Command::Shuffle { shuffle } => send_request(requests::SetShuffle(shuffle.into()))?,
    Command::Volume { volume } => send_request(requests::SetVolume(volume))?,

    Command::Seek { seek_position } => send_request(requests::Seek(seek_position))?,

    Command::Queue { command, tracks } => {
      if let Some(command) = command {
        handle_queue_command(command)?
      } else if let Some(tracks) = tracks {
        handle_queue_command(QueueCommand::Add { tracks })?
      } else {
        unreachable!("Parser requires either command or tracks")
      }
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
