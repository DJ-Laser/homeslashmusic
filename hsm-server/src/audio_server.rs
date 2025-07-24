use std::io;

use futures_concurrency::future::Race;
use message::{Message, Query};
use player::Player;
use rodio::OutputStream;
use smol::channel::{self, Receiver, Sender};

pub mod message;
mod player;

pub use player::{LoopMode, PlaybackState};

pub struct AudioServer {
  #[allow(dead_code)]
  output_stream: OutputStream,
  player: Player,
  message_rx: Receiver<Message>,
}

impl AudioServer {
  pub fn init() -> (Self, Sender<Message>) {
    let output_stream = rodio::OutputStreamBuilder::open_default_stream()
      .expect("Could not open default audio stream");

    let (message_tx, message_rx) = channel::unbounded();

    (
      Self {
        player: Player::connect_new(output_stream.mixer()),
        output_stream,
        message_rx,
      },
      message_tx,
    )
  }

  async fn handle_query(&self, query: Query) {
    let _ = match query {
      Query::PlaybackState(mut tx) => tx.send(self.player.playback_state()),
      Query::LoopMode(mut tx) => tx.send(self.player.loop_mode()),
      Query::Volume(mut tx) => tx.send(self.player.volume().await),
    };
  }

  async fn handle_messages(&self) -> Result<(), io::Error> {
    loop {
      let message = self.message_rx.recv().await.map_err(io::Error::other)?;

      match message {
        Message::Play => self.player.play(),
        Message::Pause => self.player.pause(),
        Message::Toggle => self.player.toggle_playback(),
        Message::Stop => self.player.stop(),

        Message::SetLoopMode(loop_mode) => self.player.set_loop_mode(loop_mode),
        Message::SetVolume(volume) => self.player.set_volume(volume).await,

        Message::SetTrack(path) => self
          .player
          .set_current_track(path)
          .await
          .unwrap_or_else(|e| eprintln!("Error opening track: {}", e)),

        Message::Query(query) => self.handle_query(query).await,
      }
    }
  }

  pub async fn run(&self) -> Result<(), io::Error> {
    (self.player.run(), self.handle_messages()).race().await
  }
}
