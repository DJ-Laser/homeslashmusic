use std::{
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

use async_oneshot as oneshot;
use controlled_source::{SourceEvent, wrap_source};
use decoder::TrackDecoder;
use errors::{LoadTrackError, PlayerError, SeekError};
use hsm_ipc::{LoopMode, PlaybackState, SeekPosition, Track};
use rodio::{
  Source,
  mixer::Mixer,
  queue::{SourcesQueueInput, SourcesQueueOutput, queue},
};
use smol::{
  channel::{self, Receiver, Sender},
  lock::Mutex,
};

use atomic_control_status::{AtomicLoopMode, AtomicPlaybackState};

use super::event::Event;

mod atomic_control_status;
mod controlled_source;
mod decoder;
pub mod errors;
pub mod track;

struct Controls {
  pub playback_state: AtomicPlaybackState,
  pub loop_mode: AtomicLoopMode,
  pub volume: Mutex<f32>,
  pub to_skip: AtomicUsize,
  pub position: Mutex<Duration>,
  pub seek_position: Mutex<Option<(SeekPosition, oneshot::Sender<Result<(), SeekError>>)>>,
}

impl Controls {
  pub fn new() -> Self {
    Self {
      playback_state: AtomicPlaybackState::new(PlaybackState::Stopped),
      loop_mode: AtomicLoopMode::new(LoopMode::None),
      to_skip: AtomicUsize::new(0),
      volume: Mutex::new(1.0),
      position: Mutex::new(Duration::ZERO),
      seek_position: Mutex::new(None),
    }
  }
}

pub struct Player {
  current_track: Mutex<Option<Track>>,

  source_queue: Arc<SourcesQueueInput>,
  source_count: AtomicUsize,

  controls: Arc<Controls>,
  event_tx: Sender<Event>,
  source_tx: Sender<SourceEvent>,
  source_rx: Receiver<SourceEvent>,
}

impl Player {
  pub fn connect_new(event_tx: Sender<Event>, mixer: &Mixer) -> Self {
    let (player, source) = Self::new(event_tx);
    mixer.add(source);
    player
  }

  pub fn new(event_tx: Sender<Event>) -> (Self, SourcesQueueOutput) {
    let (queue_in, queue_out) = queue(true);
    let (source_tx, source_rx) = channel::unbounded();

    (
      Self {
        current_track: Mutex::new(None),
        source_queue: queue_in,
        source_count: AtomicUsize::new(0),
        controls: Arc::new(Controls::new()),
        event_tx,
        source_tx,
        source_rx,
      },
      queue_out,
    )
  }

  fn emit(&self, event: Event) -> Result<(), PlayerError> {
    self
      .event_tx
      .try_send(event)
      .map_err(|_| PlayerError::EventChannelClosed)
  }

  pub fn playback_state(&self) -> PlaybackState {
    self.controls.playback_state.load(Ordering::Relaxed)
  }

  fn set_playback_state(&self, new_state: PlaybackState) -> Result<PlaybackState, PlayerError> {
    let prev_state = self
      .controls
      .playback_state
      .swap(new_state, Ordering::Relaxed);
    if prev_state != new_state {
      self.emit(Event::PlaybackStateChanged(new_state))?;
    }

    Ok(prev_state)
  }

  pub async fn play(&self) -> Result<(), PlayerError> {
    let prev_state = self.set_playback_state(PlaybackState::Playing)?;
    if matches!(prev_state, PlaybackState::Stopped) {
      self
        .recreate_source_queue()
        .await
        .unwrap_or_else(|error| eprintln!("Failed to resume playback: {error}"));
    }

    Ok(())
  }

  pub async fn pause(&self) -> Result<(), PlayerError> {
    // Don't un-stop playback on pause
    let prev_state = self.controls.playback_state.compare_exchange(
      PlaybackState::Playing,
      PlaybackState::Paused,
      Ordering::Relaxed,
      Ordering::Relaxed,
    );

    if let Ok(_) = prev_state {
      self.emit(Event::PlaybackStateChanged(PlaybackState::Paused))?;
    }

    Ok(())
  }

  pub async fn toggle_playback(&self) -> Result<(), PlayerError> {
    let current_state = self.controls.playback_state.load(Ordering::Acquire);
    let new_state = match current_state {
      PlaybackState::Paused | PlaybackState::Stopped => PlaybackState::Playing,
      PlaybackState::Playing => PlaybackState::Paused,
    };
    self
      .controls
      .playback_state
      .store(new_state, Ordering::Release);
    self.emit(Event::PlaybackStateChanged(new_state))
  }

  pub async fn stop(&self) -> Result<(), PlayerError> {
    self.skip(self.source_count.load(Ordering::Acquire));
    self.set_playback_state(PlaybackState::Stopped)?;
    Ok(())
  }

  pub fn loop_mode(&self) -> LoopMode {
    self.controls.loop_mode.load(Ordering::Relaxed)
  }

  pub async fn set_loop_mode(&self, loop_mode: LoopMode) -> Result<(), PlayerError> {
    let prev_mode = self.controls.loop_mode.swap(loop_mode, Ordering::Relaxed);
    if loop_mode != prev_mode {
      self.emit(Event::LoopModeChanged(loop_mode))?;
    }

    Ok(())
  }

  pub async fn volume(&self) -> f32 {
    *self.controls.volume.lock().await
  }

  pub async fn set_volume(&self, volume: f32) -> Result<(), PlayerError> {
    let clamped_volume = volume.clamp(0.0, 1.0);
    let prev_volume = {
      let mut volume_control = self.controls.volume.lock().await;
      let prev_volume = *volume_control;
      *volume_control = clamped_volume;
      prev_volume
    };

    if clamped_volume != prev_volume {
      self.emit(Event::VolumeChanged(clamped_volume))?;
    }

    Ok(())
  }

  pub async fn position(&self) -> Duration {
    *self.controls.position.lock().await
  }

  pub async fn seek(&self, seek_position: SeekPosition) -> Result<(), SeekError> {
    let (tx, rx) = oneshot::oneshot();
    *self.controls.seek_position.lock().await = Some((seek_position, tx));

    rx.await.map_err(|_| SeekError::ErrorChannelClosed)?
  }

  fn skip(&self, num_tracks: usize) {
    self.controls.to_skip.store(
      num_tracks.max(self.source_count.load(Ordering::Acquire)),
      Ordering::Release,
    );
  }

  fn wrap_source(&self, source: impl Source + 'static) -> impl Source + 'static {
    wrap_source(source, self.controls.clone(), self.source_tx.clone())
  }

  async fn recreate_source_queue(&self) -> Result<(), LoadTrackError> {
    self.skip(self.source_count.load(Ordering::Acquire));

    if let Some(track) = self.current_track.lock().await.as_ref() {
      let source = self.wrap_source(TrackDecoder::new(track.clone()).await?);
      self.source_queue.append(source);
      self.source_count.fetch_add(1, Ordering::Release);
    }

    Ok(())
  }

  pub async fn set_current_track(&self, track: Track) -> Result<(), LoadTrackError> {
    *self.current_track.lock().await = Some(track);
    self.recreate_source_queue().await?;
    self
      .controls
      .playback_state
      .store(PlaybackState::Playing, Ordering::Relaxed);

    Ok(())
  }

  pub async fn current_track(&self) -> Option<Track> {
    self.current_track.lock().await.clone()
  }

  pub async fn run(&self) -> Result<(), PlayerError> {
    loop {
      let event = self
        .source_rx
        .recv()
        .await
        .map_err(|_| PlayerError::SourceChannelClosed)?;

      if event.indicates_end() {
        let source_count = self.source_count.fetch_sub(1, Ordering::AcqRel);
        // Set state to stopped on last source finish
        if source_count == 1 {
          self.set_playback_state(PlaybackState::Stopped)?;
          *self.controls.position.lock().await = Duration::ZERO;
        }
      }

      match event {
        SourceEvent::LoopError(error) => eprintln!("Error looping source: {}", error),
        SourceEvent::Seeked(position) => self.emit(Event::Seeked(position))?,
        _ => (),
      }
    }
  }
}
