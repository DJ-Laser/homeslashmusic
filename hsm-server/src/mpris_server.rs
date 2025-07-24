use mpris_impl::MprisImpl;
use mpris_server::{Server, zbus};
use smol::{channel::Sender, future};
use thiserror::Error;

use crate::audio_server;

mod mpris_impl;

#[derive(Debug, Error)]
pub enum MprisServerError {
  #[error("Mpris server error: {0}")]
  DBus(#[from] zbus::Error),
}

pub struct MprisServer {
  server: Server<MprisImpl>,
}

impl MprisServer {
  pub const BUS_NAME: &str = "dev.djlaser.HomeSlashMusic";

  pub async fn init(
    message_tx: Sender<audio_server::message::Message>,
  ) -> Result<Self, MprisServerError> {
    let server = Server::new(Self::BUS_NAME, MprisImpl::new(message_tx)).await?;

    Ok(Self { server })
  }

  pub async fn run(&self) -> Result<(), MprisServerError> {
    loop {
      future::pending::<()>().await;
    }
  }
}
