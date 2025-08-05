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
mod track_cache;

use thiserror::Error;
use track_cache::TrackCache;

#[derive(Debug, Error)]
pub enum AudioServerError {
  #[error("AudioServer Message channel closed")]
  MessageChannelClosed,

  #[error("Internal AudioServer Error: Player Event channel closed")]
  EventChannelClosed,

  #[error(transparent)]
  PlayerError(#[from] player::errors::PlayerError),
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
        track_cache: TrackCache::new(),
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
      Query::Position(mut tx) => tx.send(self.player.position().await),
      Query::CurrentTrack(mut tx) => tx.send(self.player.current_track().await),
    };
  }

  async fn handle_message(&self, message: Message) -> Result<(), AudioServerError> {
    match message {
      Message::Play => self.player.play().await?,
      Message::Pause => self.player.pause().await?,
      Message::Toggle => self.player.toggle_playback().await?,
      Message::Stop => self.player.stop().await?,

      Message::SetLoopMode(loop_mode) => self.player.set_loop_mode(loop_mode).await?,
      Message::SetVolume(volume) => self.player.set_volume(volume).await?,

      Message::Seek(seek_position) => self
        .player
        .seek(seek_position)
        .await
        .unwrap_or_else(|error| eprintln!("Failed to seek: {}", error)),

      Message::InsertTracks {
        paths,
        position,
        mut error_tx,
      } => {
        println!("Loading tracks: {:?}", paths);
        let (tracks, errors) = self.track_cache.get_or_load_tracks(paths).await;

        for (path, error) in errors.iter() {
          eprintln!("Could not load track {path:?}: {error}")
        }

        let _ = error_tx.send(errors);

        self.player.insert_tracks(position, &tracks).await?;
        for track in tracks {
          println!("Loaded track {:?}", track.file_path());
        }
      }

      Message::ClearTracks => self.player.clear_tracks().await?,

      Message::Query(query) => self.handle_query(query).await,
    }

    Ok(())
  }

  async fn handle_messages(&self) -> Result<(), AudioServerError> {
    loop {
      let message = self
        .message_rx
        .recv()
        .await
        .map_err(|_| AudioServerError::MessageChannelClosed)?;
      if let Err(error) = self.handle_message(message).await {
        if error.is_recoverable() {
          eprintln!("{error}");
        } else {
          return Err(error);
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
      self.handle_messages(),
      self.forward_events(),
    )
      .race()
      .await
  }
}
