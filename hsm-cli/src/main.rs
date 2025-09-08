use std::io;

use clap::Parser;
use thiserror::Error;

use cli::Cli;
use commands::handle_command;

mod cli;
mod commands;
mod ipc;

#[derive(Debug, Error)]
pub enum Error {
  #[error("Could not connect to socket {path}")]
  FailedToConnectToSocket { path: String, source: io::Error },

  #[error("Error communicating with server")]
  StreamReadWrite(#[source] io::Error),

  #[error("Failed to get the working directory: {0}")]
  GetCurrentDirFailed(io::Error),

  #[error("Failed to deserialize reply from server")]
  Deserialize(#[source] serde_json::Error),

  #[error("Error: {0}")]
  Server(String),
}

fn main() -> Result<(), crate::Error> {
  let command = Cli::parse();

  handle_command(command)
}
