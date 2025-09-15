use std::{error::Error, fmt};

use super::plugin_manager::RequestJson;
use futures_concurrency::future::Race;
use hsm_ipc::Event;
use rodio::OutputStream;
use smol::channel::{Receiver, Sender};

use player::Player;

mod player;
mod request_handler;
mod track;

use thiserror::Error;
use track::TrackCache;

#[derive(Debug, Error)]
pub enum AudioServerError {
  #[error("AudioServer Message channel closed")]
  MessageChannelClosed,

  #[error(transparent)]
  PlayerError(#[from] player::PlayerError),

  #[error(transparent)]
  PluginError(Box<dyn Error>),
}

impl AudioServerError {
  pub fn is_recoverable(&self) -> bool {
    match self {
      AudioServerError::PlayerError(error) => error.is_recoverable(),
      _ => false,
    }
  }
}

pub struct AudioServer {
  #[allow(dead_code)]
  output_stream: OutputStream,
  player: Player,
  /// Mapping from cannonical path to track
  track_cache: TrackCache,

  request_data_rx: Receiver<RequestJson>,
}

impl AudioServer {
  pub fn init((request_data_rx, event_tx): (Receiver<RequestJson>, Sender<Event>)) -> Self {
    let output_stream = rodio::OutputStreamBuilder::open_default_stream()
      .expect("Could not open default audio stream");

    Self {
      player: Player::connect_new(event_tx, output_stream.mixer()),
      track_cache: TrackCache::new(),
      output_stream,

      request_data_rx,
    }
  }

  async fn handle_requests(&self) -> Result<(), AudioServerError> {
    loop {
      let (request_data, mut reply_tx) = self
        .request_data_rx
        .recv()
        .await
        .map_err(|_| AudioServerError::MessageChannelClosed)?;

      match hsm_ipc::server::handle_request(&request_data, self).await {
        Ok(reply_data) => {
          let _ = reply_tx.send(reply_data);
        }

        Err((reply_data, error)) => {
          let _ = reply_tx.send(reply_data);

          if error.is_recoverable() {
            eprintln!("{error}");
          } else {
            return Err(error);
          }
        }
      }
    }
  }

  pub async fn run(&self) -> Result<(), AudioServerError> {
    (
      async {
        self
          .player
          .run()
          .await
          .map_err(AudioServerError::PlayerError)
      },
      self.handle_requests(),
    )
      .race()
      .await
  }
}

impl fmt::Debug for AudioServer {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("AudioServer")
      .field("output_stream", &"OutputStream")
      .field("player", &self.player)
      .field("track_cache", &self.track_cache)
      .field("request_data_rx", &self.request_data_rx)
      .finish()
  }
}
