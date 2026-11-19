mod cli;
mod commands;
mod ipc;

use std::io;

use clap::Parser;
use thiserror::Error;

use cli::Cli;
use commands::handle_command;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not connect to socket {path}")]
    FailedToConnectToSocket { path: String, source: io::Error },

    #[error("Error communicating with server")]
    StreamReadWrite(#[source] io::Error),

    #[error("Failed to make ansolute path: {0}")]
    AbsolutePathFailed(io::Error),

    #[error("Failed to deserialize reply from server: {0}")]
    Deserialize(#[from] hsm_ipc::client::ParseError),

    #[error("Error when sending ipc request: {0}")]
    Server(#[from] hsm_ipc::Error),
}
fn main() -> Result<(), crate::Error> {
    let command = Cli::parse();

    handle_command(command)
}
