use std::{
  fs::File as SyncFile,
  io::{self, BufReader as SyncBufReader},
  path::Path,
  sync::Arc,
};

use rodio::{
  Decoder, Source,
  mixer::Mixer,
  queue::{SourcesQueueInput, SourcesQueueOutput, queue},
};

pub struct Player {
  track_queue: Arc<SourcesQueueInput>,
}

impl Player {
  pub fn connect_new(mixer: &Mixer) -> Self {
    let (player, source) = Self::new();
    mixer.add(source);
    player
  }

  pub fn new() -> (Self, SourcesQueueOutput) {
    let (queue_in, queue_out) = queue(true);

    (
      Self {
        track_queue: queue_in,
      },
      queue_out,
    )
  }

  async fn load_file<P: AsRef<Path>>(
    path: P,
  ) -> Result<Decoder<SyncBufReader<SyncFile>>, io::Error> {
    let path = path.as_ref().to_owned();
    let (path, file, len) = smol::unblock(|| {
      let file = SyncFile::open(&path)?;
      let len = file.metadata()?.len();

      Ok::<_, io::Error>((path, file, len))
    })
    .await?;

    let mut builder = Decoder::builder()
      .with_data(SyncBufReader::new(file))
      .with_byte_len(len);

    if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
      builder = builder.with_hint(extension);
    }

    Ok(builder.build().map_err(io::Error::other)?)
  }

  pub async fn run(&mut self) {
    loop {
      let source = Self::load_file("").await.unwrap();
      let duration = source.total_duration().unwrap();
      self.track_queue.append(source);
      smol::Timer::after(duration).await;
    }
  }
}
