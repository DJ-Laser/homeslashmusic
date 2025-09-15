use std::sync::Arc;

use audio_server::{AudioServer, AudioServerError};
use futures_concurrency::future::Race;
use hsm_plugin_ipc::IpcPlugin;
use hsm_plugin_mpris::MprisPlugin;
use plugin_manager::{PluginError, PluginManager, PluginRunner};
use signals::{SignalHandler, SignalHandlerError};
use smol::Executor;
use thiserror::Error;

mod audio_server;
mod plugin_manager;
mod signals;

#[derive(Debug, Error)]
pub enum MainError {
  #[error(transparent)]
  AudioServerError(#[from] AudioServerError),

  #[error(transparent)]
  SignalHandlerError(#[from] SignalHandlerError),

  #[error(transparent)]
  PluginError(#[from] PluginError),
}

async fn run_servers(ex: &Arc<Executor<'static>>) -> Result<(), MainError> {
  let mut signal_handler = SignalHandler::init()?;

  let (plugin_manager, audio_server_channels) = PluginManager::new(ex.clone());
  let audio_server = AudioServer::init(audio_server_channels);

  #[cfg(feature = "hsm-plugin-mpris")]
  let mpris_server: PluginRunner<MprisPlugin<_>> = plugin_manager.load_plugin().await?;

  #[cfg(feature = "hsm-plugin-ipc")]
  let ipc_server: PluginRunner<IpcPlugin<_>> = plugin_manager.load_plugin().await?;

  let server_futures = (
    async { audio_server.run().await.map_err(Into::into) },
    async { plugin_manager.run().await.map_err(Into::into) },
    #[cfg(feature = "hsm-plugin-mpris")]
    async {
      mpris_server.run().await.map_err(Into::into)
    },
    #[cfg(feature = "hsm-plugin-ipc")]
    async {
      ipc_server.run().await.map_err(Into::into)
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
