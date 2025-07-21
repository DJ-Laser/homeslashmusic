use std::io::Cursor;

use rodio::{Decoder, Sink, Source, mixer::Mixer};
use smol::fs;

pub struct Player {
  sink: Sink,
}

impl Player {
  pub fn new(mixer: &Mixer) -> Self {
    let sink = Sink::connect_new(mixer);
    Self { sink }
  }

  pub async fn run(&mut self) {
    loop {
      let bytes = fs::read("").await.unwrap();
      let source = Decoder::new(Cursor::new(bytes)).unwrap();
      let duration = source.total_duration().unwrap();
      self.sink.append(source);
      smol::Timer::after(duration).await;
    }
  }
}
