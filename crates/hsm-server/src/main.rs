mod audio;
mod cli;
mod config;
mod database;
mod ipc;
mod mpris;
mod signals;

use std::sync::Arc;

use clap::Parser as _;
use futures_concurrency::future::Race;
use hsm_ipc::Event;
use log::{error, info};
use smol::Executor;
use thiserror::Error;

use crate::{
    audio::{AudioServerError, Player, PlayerError, start_audio_server},
    cli::{Cli, Command},
    config::{config_file_path, parse_config_file},
    database::LoadDbError,
    signals::{SignalHandler, SignalHandlerError},
};
use crate::{database::DatabaseHandle, ipc::IpcServerError, mpris::MprisServerError};

#[derive(Debug, Error)]
pub enum MainError {
    #[error(transparent)]
    AudioServer(#[from] AudioServerError),

    #[error(transparent)]
    Player(#[from] PlayerError),

    #[error(transparent)]
    SignalHandler(#[from] SignalHandlerError),

    #[error(transparent)]
    Mpris(#[from] MprisServerError),

    #[error(transparent)]
    Ipc(#[from] IpcServerError),

    #[error(transparent)]
    LoadDatabse(#[from] LoadDbError),

    #[error(transparent)]
    ParseConfig(#[from] Box<kdl_config::KdlConfigError>),
}

#[derive(Debug, Clone)]
pub struct AppHandle {
    pub executor: Arc<Executor<'static>>,
    pub database: DatabaseHandle,
    pub player: Arc<Player>,
    event_rx: async_broadcast::InactiveReceiver<Event>,
}

impl AppHandle {
    pub fn subscribe(&self) -> async_broadcast::Receiver<Event> {
        self.event_rx.activate_cloned()
    }
}

async fn run_servers(executor: Arc<Executor<'static>>) -> Result<(), MainError> {
    let mut signal_handler = SignalHandler::init()?;

    let config_path = config_file_path()?;
    let config = parse_config_file(config_path.clone()).await?;

    let (mut event_tx, event_rx) = async_broadcast::broadcast(128);
    event_tx.set_overflow(true);

    let player = start_audio_server(event_tx).await?;
    let database = database::load_or_init_database(&executor, config.database).await?;

    let handle = AppHandle {
        executor,
        database,
        player: player.clone(),
        event_rx: event_rx.deactivate(),
    };

    (
        async { player.run().await.map_err(Into::into) },
        async {
            mpris::run_mpris_server(handle.clone())
                .await
                .map_err(Into::into)
        },
        async {
            ipc::run_ipc_server(handle.clone())
                .await
                .map_err(Into::into)
        },
        async {
            signal_handler.wait_for_quit().await;
            Ok(())
        },
    )
        .race()
        .await
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("symphonia", log::LevelFilter::Warn)
        .init();

    let executor: Arc<Executor<'static>> = Arc::new(Executor::new());

    let command = Cli::parse().command;

    match smol::block_on(executor.run(async {
        match command {
            None => run_servers(executor.clone()).await,
            Some(Command::Validate { path }) => {
                parse_config_file(path).await?;
                Ok(())
            }
        }
    })) {
        Ok(()) => (),
        Err(error) => error!("{error}"),
    }

    info!("hsm-server shutting down");
}
