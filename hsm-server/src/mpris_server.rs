use conversions::{as_dbus_time, as_loop_status, as_playback_status};
use futures_concurrency::future::Race;
use hsm_ipc::Event;
use mpris_impl::MprisImpl;
use mpris_server::{
  Property, Server, Signal,
  zbus::{self},
};
use smol::channel::{self, Receiver, Sender};
use thiserror::Error;

use crate::audio_server::message::Message;

mod conversions;
mod mpris_impl;

enum EventOrQuit {
  Event(Event),
  Quit,
}

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
  quit_rx: Receiver<()>,
}

impl MprisServer {
  pub const BUS_NAME: &str = "dev.djlaser.HomeSlashMusic";

  pub async fn init(
    message_tx: Sender<Message>,
    event_rx: Receiver<Event>,
  ) -> Result<Self, MprisServerError> {
    let (quit_tx, quit_rx) = channel::bounded(1);

    let server = Server::new(Self::BUS_NAME, MprisImpl::new(message_tx, quit_tx)).await?;

    Ok(Self {
      server,
      event_rx,
      quit_rx,
    })
  }

  async fn handle_event(&self, event: Event) -> Result<(), MprisServerError> {
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

  async fn await_next_event(&self) -> Result<EventOrQuit, MprisServerError> {
    (
      async { self.event_rx.recv().await.map(EventOrQuit::Event) },
      async { self.quit_rx.recv().await.map(|_| EventOrQuit::Quit) },
    )
      .race()
      .await
      .map_err(|_| MprisServerError::EventChannelClosed)
  }

  pub async fn run(&self) -> Result<(), MprisServerError> {
    loop {
      match self.await_next_event().await? {
        EventOrQuit::Event(event) => self.handle_event(event).await?,
        EventOrQuit::Quit => {
          println!("Recieved MPRIS Quit command");
          break Ok(());
        }
      }
    }
  }
}
