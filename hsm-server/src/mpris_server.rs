use hsm_ipc::{LoopMode, PlaybackState, Track};
use mpris_impl::MprisImpl;
use mpris_server::{
  LoopStatus, Metadata, PlaybackStatus, Property, Server, Signal, Time,
  zbus::{self, zvariant::ObjectPath},
};
use smol::channel::{Receiver, Sender};
use thiserror::Error;

use crate::audio_server::{event::Event, message::Message};

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
          .properties_changed([Property::PlaybackStatus(playback_status(playback_state))])
          .await?;
      }
      Event::LoopModeChanged(loop_mode) => {
        self
          .server
          .properties_changed([Property::LoopStatus(loop_status(loop_mode))])
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
            position: Time::from_micros(position.as_micros() as i64),
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

fn playback_status(playback_state: PlaybackState) -> PlaybackStatus {
  match playback_state {
    PlaybackState::Playing => PlaybackStatus::Playing,
    PlaybackState::Paused => PlaybackStatus::Paused,
    PlaybackState::Stopped => PlaybackStatus::Stopped,
  }
}

fn loop_status(loop_mode: LoopMode) -> LoopStatus {
  match loop_mode {
    LoopMode::None => LoopStatus::None,
    LoopMode::Track => LoopStatus::Track,
    LoopMode::Playlist => LoopStatus::Playlist,
  }
}

fn metadata(track: &Track) -> Metadata {
  let track_id = ObjectPath::from_static_str_unchecked("/dev/djlaser/HomeSlashMusic/DefaultTrack");

  let metadata = track.metadata().clone();
  let mut builder = Metadata::builder()
    .trackid(track_id)
    .artist(metadata.artists)
    .genre(metadata.genres)
    .comment(metadata.comments);

  if let Some(title) = metadata.title {
    builder = builder.title(title);
  }

  if let Some(album) = metadata.album {
    builder = builder.album(album);
  }

  if let Some(track_number) = metadata.track_number {
    builder = builder.track_number(track_number as i32);
  }

  if let Some(date) = metadata.date {
    builder = builder.content_created(date);
  }

  if let Some(duration) = track.audio_spec().total_duration {
    builder = builder.length(Time::from_micros(duration.as_micros() as i64));
  }

  builder.build()
}
