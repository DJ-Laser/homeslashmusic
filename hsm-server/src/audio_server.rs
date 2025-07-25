use event::Event;
use futures_concurrency::future::Race;
use message::{Message, Query};
use player::Player;
use rodio::OutputStream;
use smol::{
  channel::{self, Receiver, Sender},
  lock::Mutex,
};

pub mod event;
pub mod message;
mod player;

pub use player::{LoopMode, PlaybackState};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioServerError {
  #[error("AudioServer Message channel closed")]
  MessageChannelClosed,

  #[error("Internal AudioServer Error: Player Event channel closed")]
  EventChannelClosed,

  #[error(transparent)]
  PlayerError(#[from] player::errors::PlayerError),
}

pub struct AudioServer {
  #[allow(dead_code)]
  output_stream: OutputStream,
  player: Player,
  message_rx: Receiver<Message>,
  player_event_rx: Receiver<Event>,
  event_broadcast_tx: Mutex<Vec<Sender<Event>>>,
}

impl AudioServer {
  pub fn init() -> (Self, Sender<Message>) {
    let output_stream = rodio::OutputStreamBuilder::open_default_stream()
      .expect("Could not open default audio stream");

    let (player_event_tx, player_event_rx) = channel::unbounded();
    let (message_tx, message_rx) = channel::unbounded();

    (
      Self {
        player: Player::connect_new(player_event_tx, output_stream.mixer()),
        output_stream,
        message_rx,
        player_event_rx,
        event_broadcast_tx: Mutex::new(Vec::new()),
      },
      message_tx,
    )
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

  async fn handle_query(&self, query: Query) {
    let _ = match query {
      Query::PlaybackState(mut tx) => tx.send(self.player.playback_state()),
      Query::LoopMode(mut tx) => tx.send(self.player.loop_mode()),
      Query::Volume(mut tx) => tx.send(self.player.volume().await),
    };
  }

  async fn handle_messages(&self) -> Result<(), AudioServerError> {
    loop {
      let message = self
        .message_rx
        .recv()
        .await
        .map_err(|_| AudioServerError::MessageChannelClosed)?;

      match message {
        Message::Play => self.player.play()?,
        Message::Pause => self.player.pause()?,
        Message::Toggle => self.player.toggle_playback()?,
        Message::Stop => self.player.stop()?,

        Message::SetLoopMode(loop_mode) => self.player.set_loop_mode(loop_mode)?,
        Message::SetVolume(volume) => self.player.set_volume(volume).await?,

        Message::SetTrack(path) => self
          .player
          .set_current_track(path)
          .await
          .unwrap_or_else(|e| eprintln!("Error opening track: {}", e)),

        Message::Query(query) => self.handle_query(query).await,
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
      self.handle_messages(),
      self.forward_events(),
    )
      .race()
      .await
  }
}
