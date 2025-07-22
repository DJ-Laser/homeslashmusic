use std::{
  fs::File as SyncFile,
  io::{self, BufReader as SyncBufReader},
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
};

use controlled_source::{SourceEvent, wrap_source};
use rodio::{
  Decoder, Source,
  mixer::Mixer,
  queue::{SourcesQueueInput, SourcesQueueOutput, queue},
};
use smol::{
  channel::{self, Receiver, Sender},
  lock::Mutex,
};

use control_status::{AtomicLoopMode, AtomicPlaybackState};
pub use control_status::{LoopMode, PlaybackState};

mod control_status;
mod controlled_source;

struct Controls {
  pub playback_state: AtomicPlaybackState,
  pub loop_mode: AtomicLoopMode,
  pub to_skip: AtomicUsize,
  pub volume: Mutex<f32>,
}

impl Controls {
  pub fn new() -> Self {
    Self {
      playback_state: AtomicPlaybackState::new(PlaybackState::Stopped),
      loop_mode: AtomicLoopMode::new(LoopMode::Track),
      to_skip: AtomicUsize::new(0),
      volume: Mutex::new(1.0),
    }
  }
}

pub struct Player {
  track_queue: Arc<SourcesQueueInput>,
  controls: Arc<Controls>,
  source_tx: Sender<SourceEvent>,
  source_rx: Receiver<SourceEvent>,
}

impl Player {
  pub fn connect_new(mixer: &Mixer) -> Self {
    let (player, source) = Self::new();
    mixer.add(source);
    player
  }

  pub fn new() -> (Self, SourcesQueueOutput) {
    let (queue_in, queue_out) = queue(true);
    let (source_tx, source_rx) = channel::unbounded();

    (
      Self {
        track_queue: queue_in,
        controls: Arc::new(Controls::new()),
        source_tx,
        source_rx,
      },
      queue_out,
    )
  }

  async fn load_file<P: AsRef<Path>>(&self, path: P) -> Result<impl Source + 'static, io::Error> {
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

    let decoder = builder.build().map_err(io::Error::other)?;
    Ok(wrap_source(
      decoder,
      self.controls.clone(),
      self.source_tx.clone(),
    ))
  }

  pub async fn run(&self) -> Result<(), io::Error> {
    let add_source = async || {
      let source = self
        .load_file("/home/dj_laser/Music/get out!.mp3")
        .await
        .unwrap();
      self.track_queue.append(source);
    };

    add_source().await;

    loop {
      let event = match self.source_rx.recv().await {
        Ok(event) => event,
        Err(_) => unreachable!("Source event channel should never close while in use"),
      };

      match event {
        SourceEvent::LoopError(error) => eprintln!("Error looping source: {}", error),
        _ => (),
      }
    }
  }

  pub fn set_playing(&self, playing: bool) {
    if playing {
      self
        .controls
        .playback_state
        .store(PlaybackState::Playing, Ordering::Relaxed);
    } else {
      // Don't un-stop playback on pause
      let _ = self.controls.playback_state.compare_exchange(
        PlaybackState::Playing,
        PlaybackState::Paused,
        Ordering::Relaxed,
        Ordering::Relaxed,
      );
    }
  }

  pub fn toggle_playback(&self) {
    let current_state = self.controls.playback_state.load(Ordering::Relaxed);
    let new_state = match current_state {
      PlaybackState::Paused | PlaybackState::Stopped => PlaybackState::Playing,
      PlaybackState::Playing => PlaybackState::Paused,
    };
    self
      .controls
      .playback_state
      .store(new_state, Ordering::Relaxed);
  }
}
