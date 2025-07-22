use mpris_impl::MprisImpl;
use mpris_server::Server;
use smol::{channel::Sender, future, io};

use crate::audio_server;

mod mpris_impl;

pub struct MprisServer {
  server: Server<MprisImpl>,
}

impl MprisServer {
  pub const BUS_NAME: &str = "dev.djlaser.HomeSlashMusic";

  pub async fn init(message_tx: Sender<audio_server::message::Message>) -> io::Result<Self> {
    let server = Server::new(Self::BUS_NAME, MprisImpl::new(message_tx))
      .await
      .map_err(io::Error::other)?;

    Ok(Self { server })
  }

  pub async fn run(&self) -> Result<(), io::Error> {
    loop {
      future::pending::<()>().await;
    }
  }
}
