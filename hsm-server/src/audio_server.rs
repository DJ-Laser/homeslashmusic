use std::error::Error;

use super::plugin_manager::{PluginManager, RequestJson};
use futures_concurrency::future::Race;
use rodio::OutputStream;
use smol::channel::{self, Receiver};

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
  plugin_manager: PluginManager,
}

impl AudioServer {
  pub fn init() -> Self {
    let output_stream = rodio::OutputStreamBuilder::open_default_stream()
      .expect("Could not open default audio stream");

    let (request_data_tx, request_data_rx) = channel::unbounded();
    let (plugin_manager, player_event_tx) = PluginManager::new(request_data_tx);

    Self {
      player: Player::connect_new(player_event_tx, output_stream.mixer()),
      track_cache: TrackCache::new(),
      output_stream,

      request_data_rx,
      plugin_manager,
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

  pub fn plugin_manager(&self) -> &PluginManager {
    &self.plugin_manager
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
