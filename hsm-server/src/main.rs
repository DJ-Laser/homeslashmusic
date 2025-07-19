use ctrlc::CtrlCHandler;
use ipc::IpcServer;

mod ctrlc;
mod ipc;

fn main() {
  let ex = smol::Executor::new();
  let ctrlc = CtrlCHandler::init();

  let stream_handle =
    rodio::OutputStreamBuilder::open_default_stream().expect("Could not open default audio stream");

  let channel = smol::channel::unbounded::<usize>();

  let ipc_server = ex.spawn(async { IpcServer::new().run().await.unwrap() });

  smol::block_on(ex.run(async {
    ctrlc.wait_for_ctrlc().await;
    ipc_server.cancel().await;
  }));
}
