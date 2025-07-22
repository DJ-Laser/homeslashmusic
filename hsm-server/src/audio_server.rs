use std::io;

use futures_concurrency::future::Race;
use message::{Message, PlaybackControl};
use player::Player;
use rodio::OutputStream;
use smol::channel::{self, Receiver, Sender};

pub mod message;
mod player;

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

  async fn handle_messages(&self) -> Result<(), io::Error> {
    loop {
      let message = self.message_rx.recv().await.map_err(io::Error::other)?;

      match message {
        Message::Playback(message) => match message {
          PlaybackControl::Play => self.player.set_playing(true),
          PlaybackControl::Pause => self.player.set_playing(false),
          PlaybackControl::Toggle => self.player.toggle_playback(),
        },
      }
    }
  }

  pub async fn run(&self) -> Result<(), io::Error> {
    (self.player.run(), self.handle_messages()).race().await
  }
}
