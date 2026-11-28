mod cli;
mod commands;
mod ipc;
mod tui;

use std::{io, path::PathBuf};

use clap::Parser;
use thiserror::Error;

use cli::Cli;
use commands::handle_command;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not connect to socket {path}")]
    FailedToConnectToSocket { path: PathBuf, source: io::Error },

    #[error("Error communicating with server")]
    StreamReadWrite(#[source] io::Error),

    #[error("Failed to make ansolute path: {0}")]
    AbsolutePathFailed(io::Error),

    #[error("Failed to deserialize reply from server: {0}")]
    Deserialize(#[from] hsm_ipc::client::ParseError),

    #[error("Error when sending ipc request: {0}")]
    Server(#[from] hsm_ipc::Error),

    #[error(transparent)]
    Terminal(io::Error),
}
fn main() -> Result<(), crate::Error> {
    let command = Cli::parse();

    if let Some(command) = command.command {
        return handle_command(command);
    }

    ratatui::run(tui::app).map_err(Error::Terminal)?;

    Ok(())
}
