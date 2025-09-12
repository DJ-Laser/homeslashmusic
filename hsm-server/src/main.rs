use std::{error::Error, sync::Arc};

use audio_server::AudioServer;
use futures_concurrency::future::Race;
use ipc::IpcServer;
use signals::SignalHandler;
use smol::Executor;

mod audio_server;
mod ipc;
mod signals;

async fn run_servers(ex: &Arc<Executor<'static>>) -> Result<(), Box<dyn Error>> {
  let mut signal_handler = SignalHandler::init()?;

  let audio_server = AudioServer::init();

  let ipc_server = IpcServer::new(audio_server.request_sender(), ex.clone())?;
  /*let mpris_server = MprisServer::init(
    message_tx.clone(),
    audio_server.register_event_listener().await,
  )
  .await?;*/

  let server_futures = (
    async move { ipc_server.run().await.map_err(Into::<Box<dyn Error>>::into) },
    async { audio_server.run().await.map_err(Into::into) },
    //async { mpris_server.run().await.map_err(Into::into) },
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
