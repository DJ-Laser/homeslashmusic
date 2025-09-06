use conversions::{as_dbus_time, as_loop_status, as_playback_status};
use mpris_impl::MprisImpl;
use mpris_server::{
  Property, Server, Signal,
  zbus::{self},
};
use smol::channel::{Receiver, Sender};
use thiserror::Error;

use crate::audio_server::{event::Event, message::Message};

mod conversions;
mod mpris_impl;

#[derive(Debug, Error)]
pub enum MprisServerError {
  #[error("Mpris server error: {0}")]
  DBus(#[from] zbus::Error),

  #[error("Event channel closed")]
  EventChannelClosed,
}

pub struct MprisServer {
  server: Server<MprisImpl>,
  event_rx: Receiver<Event>,
}

impl MprisServer {
  pub const BUS_NAME: &str = "dev.djlaser.HomeSlashMusic";

  pub async fn init(
    message_tx: Sender<Message>,
    event_rx: Receiver<Event>,
  ) -> Result<Self, MprisServerError> {
    let server = Server::new(Self::BUS_NAME, MprisImpl::new(message_tx)).await?;

    Ok(Self { server, event_rx })
  }

  async fn recieve_event(&self, event: Event) -> Result<(), MprisServerError> {
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

  pub async fn run(&self) -> Result<(), MprisServerError> {
    loop {
      let event = self
        .event_rx
        .recv()
        .await
        .map_err(|_| MprisServerError::EventChannelClosed)?;

      self.recieve_event(event).await?;
    }
  }
}
