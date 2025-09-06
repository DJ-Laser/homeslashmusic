use std::sync::Arc;

use audio_server::AudioServer;
use futures_concurrency::future::Race;
use ipc::IpcServer;
use mpris_server::MprisServer;
use signals::SignalHandler;
use smol::Executor;

mod audio_server;
mod ipc;
mod mpris_server;
mod signals;

fn main() {
  let ex: Arc<Executor<'static>> = Arc::new(Executor::new());

  smol::block_on(ex.run(async {
    let mut signal_handler = SignalHandler::init().unwrap();

    let (audio_server, message_tx) = AudioServer::init();
    let mpris_server = MprisServer::init(
      message_tx.clone(),
      audio_server.register_event_listener().await,
    )
    .await
    .unwrap();
    let ipc_server = IpcServer::new(message_tx.clone(), ex.clone()).unwrap();

    (
      async move { ipc_server.run().await.unwrap() },
      async { audio_server.run().await.unwrap() },
      async { mpris_server.run().await.unwrap() },
      signal_handler.wait_for_quit(),
    )
      .race()
      .await;
  }));
}
