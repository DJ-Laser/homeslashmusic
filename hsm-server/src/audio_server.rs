use futures_concurrency::future::Race;
use hsm_ipc::Event;
use request_handler::RequestJson;
use rodio::OutputStream;
use smol::{
  channel::{self, Receiver, Sender},
  lock::Mutex,
};

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

  #[error("Internal AudioServer Error: Player Event channel closed")]
  EventChannelClosed,

  #[error(transparent)]
  PlayerError(#[from] player::PlayerError),
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

  request_data_tx: Sender<RequestJson>,
  request_data_rx: Receiver<RequestJson>,

  player_event_rx: Receiver<Event>,
  event_broadcast_tx: Mutex<Vec<Sender<Event>>>,
}

impl AudioServer {
  pub fn init() -> Self {
    let output_stream = rodio::OutputStreamBuilder::open_default_stream()
      .expect("Could not open default audio stream");

    let (player_event_tx, player_event_rx) = channel::unbounded();
    let (request_data_tx, request_data_rx) = channel::unbounded();

    Self {
      player: Player::connect_new(player_event_tx, output_stream.mixer()),
      track_cache: TrackCache::new(),
      output_stream,

      request_data_tx,
      request_data_rx,

      player_event_rx,
      event_broadcast_tx: Mutex::new(Vec::new()),
    }
  }

  pub async fn register_event_listener(&self) -> Receiver<Event> {
    let (event_tx, event_rx) = channel::unbounded();
    self.event_broadcast_tx.lock().await.push(event_tx);
    event_rx
  }

  async fn broadcast(&self, event: Event) {
    self.event_broadcast_tx.lock().await.retain(|tx| {
      // Remove closed channels
      tx.try_send(event.clone()).is_ok()
    });
  }

  async fn forward_events(&self) -> Result<(), AudioServerError> {
    loop {
      let event = self
        .player_event_rx
        .recv()
        .await
        .map_err(|_| AudioServerError::EventChannelClosed)?;

      self.broadcast(event).await;
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
          reply_tx.send(reply_data);
        }

        Err((reply_data, error)) => {
          reply_tx.send(reply_data);

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
      self.forward_events(),
    )
      .race()
      .await
  }
}
