use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check a config file for errors.
    /// Returns a non-zero exit code if the config file is invalid.
    Validate {
        /// Path to a hsm-server config file
        path: PathBuf,
    },
}
