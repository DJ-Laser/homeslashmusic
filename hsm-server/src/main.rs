use std::sync::Arc;

use audio_server::AudioServer;
use ctrlc::CtrlCHandler;
use futures_concurrency::future::Race;
use ipc::IpcServer;
use mpris_server::MprisServer;
use smol::Executor;

mod audio_server;
mod ctrlc;
mod ipc;
mod mpris_server;

fn main() {
  let ex: Arc<Executor<'static>> = Arc::new(Executor::new());

  smol::block_on(ex.run(async {
    let ctrlc = CtrlCHandler::init();

    let (audio_server, message_tx) = AudioServer::init();
    let ipc_server = IpcServer::new(message_tx.clone(), ex.clone());
    let mpris_server = MprisServer::init(message_tx.clone()).await.unwrap();

    (
      async move { ipc_server.run().await.unwrap() },
      async { audio_server.run().await.unwrap() },
      async { mpris_server.run().await.unwrap() },
      ctrlc.wait_for_ctrlc(),
    )
      .race()
      .await;
  }));
}
