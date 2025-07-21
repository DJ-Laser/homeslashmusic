use player::Player;
use rodio::OutputStream;

mod player;

pub struct AudioServer {
  #[allow(dead_code)]
  output_stream: OutputStream,
  player: Player,
}

impl AudioServer {
  pub fn init() -> Self {
    let output_stream = rodio::OutputStreamBuilder::open_default_stream()
      .expect("Could not open default audio stream");

    Self {
      player: Player::new(output_stream.mixer()),
      output_stream,
    }
  }

  pub async fn run(&mut self) {
    self.player.run().await;
  }
}
