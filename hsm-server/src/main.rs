use std::sync::Arc;

use audio_server::{AudioServer, AudioServerError};
use futures_concurrency::future::Race;
use hsm_plugin_mpris::MprisPlugin;
use ipc::{IpcServer, IpcServerError};
use plugin_manager::{PluginError, PluginRunner};
use signals::{SignalHandler, SignalHandlerError};
use smol::Executor;
use thiserror::Error;

mod audio_server;
mod ipc;
mod plugin_manager;
mod signals;

#[derive(Debug, Error)]
pub enum MainError {
  #[error(transparent)]
  AudioServerError(#[from] AudioServerError),

  #[error(transparent)]
  SignalHandlerError(#[from] SignalHandlerError),

  #[error(transparent)]
  IpcServerError(#[from] IpcServerError),

  #[error(transparent)]
  PluginError(#[from] PluginError),
}

async fn run_servers(ex: &Arc<Executor<'static>>) -> Result<(), MainError> {
  let mut signal_handler = SignalHandler::init()?;

  let audio_server = AudioServer::init();
  let plugin_manager = audio_server.plugin_manager();

  let ipc_server = IpcServer::new(plugin_manager.request_sender(), ex.clone())?;

  #[cfg(feature = "hsm-plugin-mpris")]
  let mpris_server: PluginRunner<MprisPlugin<_>> = plugin_manager.load_plugin().await?;

  let server_futures = (
    async move { ipc_server.run().await.map_err(Into::<MainError>::into) },
    async { audio_server.run().await.map_err(Into::into) },
    async { plugin_manager.run().await.map_err(Into::into) },
    #[cfg(feature = "hsm-plugin-mpris")]
    async {
      mpris_server.run().await.map_err(Into::into)
    },
    async {
      signal_handler.wait_for_quit().await;
      Ok(())
    },
  );

  server_futures.race().await
}

fn main() {
  let ex: Arc<Executor<'static>> = Arc::new(Executor::new());
  match smol::block_on(ex.run(run_servers(&ex))) {
    Ok(()) => (),
    Err(error) => eprintln!("{error}"),
  }

  println!("hsm-server shutting down");
}
