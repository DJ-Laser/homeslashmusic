use std::sync::Arc;

use conversions::{as_dbus_time, as_loop_status, as_playback_status};
use hsm_ipc::Event;
use hsm_plugin::{Plugin, RequestSender};
use mpris_impl::MprisImpl;
use mpris_server::{
  Property, Server, Signal,
  zbus::{self},
};
use smol::{
  Executor,
  channel::{self, Receiver},
};
use thiserror::Error;

mod conversions;
mod mpris_impl;

#[derive(Debug, Error)]
pub enum MprisServerError {
  #[error("Mpris server error: {0}")]
  DBus(#[from] zbus::Error),

  #[error("Event channel closed")]
  EventChannelClosed,
}

pub struct MprisPlugin<Tx> {
  server: Server<MprisImpl<Tx>>,

  quit_rx: Receiver<()>,
}

impl<Tx> MprisPlugin<Tx> {
  pub const BUS_NAME: &str = "dev.djlaser.HomeSlashMusic";
}

impl<'ex, Tx: RequestSender + Send + Sync + 'static> Plugin<'ex, Tx> for MprisPlugin<Tx> {
  type Error = MprisServerError;

  async fn init(request_tx: Tx, _ex: Arc<Executor<'ex>>) -> Result<Self, Self::Error> {
    let (quit_tx, quit_rx) = channel::bounded(1);

    let server = Server::new(Self::BUS_NAME, MprisImpl::new(request_tx, quit_tx)).await?;

    Ok(Self { server, quit_rx })
  }

  async fn on_event(&self, event: Event) -> Result<(), Self::Error> {
    match event {
      Event::PlaybackStateChanged(playback_state) => {
        self
          .server
          .properties_changed([Property::PlaybackStatus(as_playback_status(playback_state))])
          .await?;
      }
      Event::LoopModeChanged(loop_mode) => {
        self
          .server
          .properties_changed([Property::LoopStatus(as_loop_status(loop_mode))])
          .await?;
      }
      Event::ShuffleChanged(shuffle) => {
        self
          .server
          .properties_changed([Property::Shuffle(shuffle)])
          .await?;
      }
      Event::VolumeChanged(volume) => {
        self
          .server
          .properties_changed([Property::Volume(volume.into())])
          .await?;
      }
      Event::Seeked(position) => {
        self
          .server
          .emit(Signal::Seeked {
            position: as_dbus_time(position),
          })
          .await?;
      }
    }

    Ok(())
  }

  async fn run(&self) -> Result<(), Self::Error> {
    let _ = self.quit_rx.recv().await;
    println!("Recieved MPRIS Quit command");

    Ok(())
  }
}
