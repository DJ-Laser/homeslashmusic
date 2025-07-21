use std::sync::{Arc, OnceLock};

use audio_server::AudioServer;
use ctrlc::CtrlCHandler;
use ipc::IpcServer;
use smol::Executor;

mod audio_server;
mod ctrlc;
mod ipc;

fn main() {
  static EX: OnceLock<Arc<Executor>> = OnceLock::new();
  EX.get_or_init(|| Arc::new(Executor::new()));

  let ex: &'static Arc<Executor<'static>> = EX.get().unwrap();
  let ctrlc = CtrlCHandler::init();

  let ipc_server = ex.spawn(async { IpcServer::new(ex.clone()).run().await.unwrap() });
  let audio_server = ex.spawn(async { AudioServer::init().run().await });

  smol::block_on(ex.run(async {
    ctrlc.wait_for_ctrlc().await;
    ipc_server.cancel().await;
    audio_server.cancel().await;
  }));
}
