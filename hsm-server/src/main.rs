use std::sync::Arc;

use audio_server::AudioServer;
use ctrlc::CtrlCHandler;
use futures_concurrency::future::Race;
use ipc::IpcServer;
use smol::Executor;

mod audio_server;
mod ctrlc;
mod ipc;

fn main() {
  let ex: Arc<Executor<'static>> = Arc::new(Executor::new());

  smol::block_on(ex.run(async {
    let ctrlc = CtrlCHandler::init();

    let mut ipc_server = IpcServer::new(ex.clone());
    let mut audio_server = AudioServer::init();

    (
      async { ipc_server.run().await.unwrap() },
      async { audio_server.run().await },
      ctrlc.wait_for_ctrlc(),
    )
      .race()
      .await;
  }));
}
