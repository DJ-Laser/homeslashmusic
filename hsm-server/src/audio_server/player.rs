use std::{
  fs::File as SyncFile,
  io::BufReader as SyncBufReader,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
};

use controlled_source::{SourceEvent, wrap_source};
use errors::{LoadTrackError, PlayerError};
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
pub mod errors;

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
      loop_mode: AtomicLoopMode::new(LoopMode::None),
      to_skip: AtomicUsize::new(0),
      volume: Mutex::new(1.0),
    }
  }
}

pub struct Player {
  source_queue: Arc<SourcesQueueInput>,
  source_count: AtomicUsize,

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
        source_queue: queue_in,
        source_count: AtomicUsize::new(0),
        controls: Arc::new(Controls::new()),
        source_tx,
        source_rx,
      },
      queue_out,
    )
  }

  async fn load_file(&self, path: PathBuf) -> Result<impl Source + 'static, LoadTrackError> {
    let (path, file, len) = smol::unblock(|| {
      let file = SyncFile::open(&path).map_err(|source| LoadTrackError::FileNotFound {
        path: path.clone(),
        source,
      })?;

      let metadata = file
        .metadata()
        .map_err(|source| LoadTrackError::MetadataFailed {
          path: path.clone(),
          source,
        })?;

      Ok::<_, errors::LoadTrackError>((path, file, metadata.len()))
    })
    .await?;

    let mut builder = Decoder::builder()
      .with_data(SyncBufReader::new(file))
      .with_byte_len(len);

    if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
      builder = builder.with_hint(extension);
    }

    let decoder = builder.build()?;
    Ok(wrap_source(
      decoder,
      self.controls.clone(),
      self.source_tx.clone(),
    ))
  }

  pub async fn run(&self) -> Result<(), PlayerError> {
    loop {
      let event = self
        .source_rx
        .recv()
        .await
        .map_err(|_| PlayerError::SourceEventChannelClosed)?;

      if event.indicates_end() {
        let source_count = self.source_count.fetch_sub(1, Ordering::AcqRel);
        // Set state to stopped on last source finish
        if source_count == 1 {
          self
            .controls
            .playback_state
            .store(PlaybackState::Stopped, Ordering::Relaxed);
        }
      }

      match event {
        SourceEvent::LoopError(error) => eprintln!("Error looping source: {}", error),
        _ => (),
      }
    }
  }

  pub fn playback_state(&self) -> PlaybackState {
    self.controls.playback_state.load(Ordering::Relaxed)
  }

  pub fn play(&self) {
    self
      .controls
      .playback_state
      .store(PlaybackState::Playing, Ordering::Relaxed);
  }

  pub fn pause(&self) {
    // Don't un-stop playback on pause
    let _ = self.controls.playback_state.compare_exchange(
      PlaybackState::Playing,
      PlaybackState::Paused,
      Ordering::Relaxed,
      Ordering::Relaxed,
    );
  }

  pub fn toggle_playback(&self) {
    let current_state = self.controls.playback_state.load(Ordering::Acquire);
    let new_state = match current_state {
      PlaybackState::Paused | PlaybackState::Stopped => PlaybackState::Playing,
      PlaybackState::Playing => PlaybackState::Paused,
    };
    self
      .controls
      .playback_state
      .store(new_state, Ordering::Release);
  }

  pub fn stop(&self) {
    self.skip(self.source_count.load(Ordering::Acquire));
  }

  pub fn loop_mode(&self) -> LoopMode {
    self.controls.loop_mode.load(Ordering::Relaxed)
  }

  pub fn set_loop_mode(&self, loop_mode: LoopMode) {
    self.controls.loop_mode.store(loop_mode, Ordering::Relaxed);
  }

  pub async fn volume(&self) -> f32 {
    *self.controls.volume.lock().await
  }

  pub async fn set_volume(&self, volume: f32) {
    *self.controls.volume.lock().await = volume
  }

  fn skip(&self, num_tracks: usize) {
    self.controls.to_skip.store(
      num_tracks.max(self.source_count.load(Ordering::Acquire)),
      Ordering::Release,
    );
  }

  pub async fn set_current_track(&self, path: PathBuf) -> Result<(), LoadTrackError> {
    let source = self.load_file(path).await?;
    self.skip(self.source_count.load(Ordering::Acquire));
    self.source_queue.append(source);
    self
      .controls
      .playback_state
      .store(PlaybackState::Playing, Ordering::Relaxed);
    self.source_count.fetch_add(1, Ordering::Release);
    Ok(())
  }
}
