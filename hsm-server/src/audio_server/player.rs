use std::{
  mem,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

use async_oneshot as oneshot;
use controlled_source::{SeekError, SourceEvent, wrap_source};
use decoder::TrackDecoder;
use hsm_ipc::{InsertPosition, LoopMode, PlaybackState, SeekPosition, Track};
use output::NextSourceState;
use rand::seq::SliceRandom;
use rodio::{Source, mixer::Mixer};
use smol::{
  channel::{self, Receiver, Sender},
  lock::Mutex,
};

use atomic_control_status::{AtomicLoopMode, AtomicPlaybackState};
use thiserror::Error;

use super::{event::Event, track::LoadTrackError};
pub use output::PlayerAudioOutput;

mod atomic_control_status;
mod controlled_source;
mod decoder;
mod output;

struct Controls {
  pub playback_state: AtomicPlaybackState,
  pub loop_mode: AtomicLoopMode,
  pub volume: Mutex<f32>,
  pub to_skip: AtomicUsize,
  pub position: Mutex<Duration>,
  pub seek_position: Mutex<Option<(SeekPosition, oneshot::Sender<Result<(), SeekError>>)>>,
  pub next_source: Mutex<NextSourceState>,
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
      next_source: Mutex::new(NextSourceState::None),
    }
  }
}

#[derive(Debug, Error)]
pub enum PlayerError {
  /// Should never happen since the player managers both ends of the channel
  #[error("Internal Player Error: SourceEvent channel closed")]
  SourceChannelClosed,

  /// Since we use an unbounded channel, an error means it must be closed
  #[error("Event channel closed")]
  EventChannelClosed,

  #[error("Failed to load track: {0}")]
  LoadTrack(#[from] LoadTrackError),

  #[error("Shuffle failed, could not determine new current track position")]
  ShuffleFailedNoCurrentTrack,

  #[error("failed to seek: ")]
  SeekFailed(#[from] SeekError),
}

impl PlayerError {
  pub fn is_recoverable(&self) -> bool {
    match self {
      Self::LoadTrack(_) => true,
      Self::SeekFailed(_) => true,
      Self::ShuffleFailedNoCurrentTrack => true,
      _ => false,
    }
  }
}

pub struct Player {
  track_list: Mutex<Vec<Arc<Track>>>,
  current_index: AtomicUsize,
  shuffle_order: Mutex<Option<Vec<usize>>>,

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

  pub fn new(event_tx: Sender<Event>) -> (Self, PlayerAudioOutput) {
    let (source_tx, source_rx) = channel::unbounded();

    let player = Self {
      track_list: Mutex::new(Vec::new()),
      current_index: AtomicUsize::new(0),
      shuffle_order: Mutex::new(None),

      controls: Arc::new(Controls::new()),
      event_tx,
      source_tx,
      source_rx,
    };

    let audio_source = PlayerAudioOutput::new(player.controls.clone());

    (player, audio_source)
  }

  fn emit(&self, event: Event) -> Result<(), PlayerError> {
    self
      .event_tx
      .try_send(event)
      .map_err(|_| PlayerError::EventChannelClosed)
  }

  fn wrap_source(&self, source: impl Source + 'static) -> impl Source + 'static {
    wrap_source(source, self.controls.clone(), self.source_tx.clone())
  }

  async fn clear_source_queue(&self) {
    let prev_source = self.controls.next_source.lock().await.clear();

    if !matches!(prev_source, NextSourceState::None) {
      self.controls.to_skip.fetch_add(1, Ordering::AcqRel);
    }
  }

  async fn queue_track(&self, track: &Arc<Track>) -> Result<(), LoadTrackError> {
    let source = self.wrap_source(TrackDecoder::new(track.as_ref().clone()).await?);
    *self.controls.next_source.lock().await = NextSourceState::Queued(Box::new(source));

    Ok(())
  }

  async fn get_shuffled_index(&self) -> usize {
    let current_index = self.current_index.load(Ordering::Acquire);
    self
      .shuffle_order
      .lock()
      .await
      .as_ref()
      .and_then(|order| order.get(current_index).cloned())
      .unwrap_or(current_index)
  }

  /// Returns true if there was a current track to queue
  async fn requeue_current_track(&self) -> Result<bool, LoadTrackError> {
    self.clear_source_queue().await;
    let tracks = self.track_list.lock().await;
    if tracks.len() == 0 {
      return Ok(false);
    }

    let track_index = self.get_shuffled_index().await;
    self.queue_track(&tracks[track_index]).await?;

    Ok(true)
  }

  async fn shuffle_tracks(&self) -> Result<(), PlayerError> {
    let num_tracks = self.track_list.lock().await.len();
    let current_track_index = self.get_shuffled_index().await;
    if num_tracks == 0 {
      *self.shuffle_order.lock().await = Some(Vec::new());
      return Ok(());
    }

    let mut shuffle_order: Vec<_> = (0..num_tracks).collect();
    shuffle_order.shuffle(&mut rand::rng());

    let new_current_index = shuffle_order
      .iter()
      .position(|track_index| *track_index == current_track_index)
      .ok_or(PlayerError::ShuffleFailedNoCurrentTrack)?;
    self
      .current_index
      .store(new_current_index, Ordering::Release);

    *self.shuffle_order.lock().await = Some(shuffle_order);
    println!("Shuffled track order");

    Ok(())
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
      println!("Setting playback state to {new_state:?}")
    }

    Ok(prev_state)
  }

  pub async fn play(&self) -> Result<(), PlayerError> {
    if matches!(
      self.controls.playback_state.load(Ordering::Acquire),
      PlaybackState::Stopped
    ) {
      let had_tracks = self.requeue_current_track().await?;
      if !had_tracks {
        return Ok(());
      }
    }

    self.set_playback_state(PlaybackState::Playing)?;

    Ok(())
  }

  pub async fn pause(&self) -> Result<(), PlayerError> {
    let prev_state = self.controls.playback_state.load(Ordering::Acquire);

    // Don't un-stop playback on pause
    if matches!(prev_state, PlaybackState::Playing) {
      self.set_playback_state(PlaybackState::Paused)?;
    }

    Ok(())
  }

  pub async fn toggle_playback(&self) -> Result<(), PlayerError> {
    let current_state = self.controls.playback_state.load(Ordering::Acquire);
    match current_state {
      PlaybackState::Paused | PlaybackState::Stopped => self.play().await?,
      PlaybackState::Playing => self.pause().await?,
    }

    Ok(())
  }

  pub async fn stop(&self) -> Result<(), PlayerError> {
    self.clear_source_queue().await;
    self.set_playback_state(PlaybackState::Stopped)?;
    *self.controls.position.lock().await = Duration::ZERO;
    Ok(())
  }

  async fn stop_or_wrap_track(
    &self,
    tracks: &Vec<Arc<Track>>,
    reverse: bool,
  ) -> Result<(), PlayerError> {
    let printed_position = if reverse { "beginning" } else { "end" };
    let printed_loop_position = if reverse { "end" } else { "beginning" };

    let new_index = if reverse { tracks.len() - 1 } else { 0 };

    self.current_index.store(new_index, Ordering::Release);

    if tracks.len() == 0
      || !matches!(
        self.controls.loop_mode.load(Ordering::Acquire),
        LoopMode::Playlist
      )
    {
      println!("Track list reached {printed_position}, stopping");
      self.stop().await?;
    } else {
      println!("Track list reached {printed_position}, looping to {printed_loop_position}");
      self.queue_track(&tracks[new_index]).await?;
    };

    Ok(())
  }

  pub async fn go_to_next_track(&self) -> Result<(), PlayerError> {
    let tracks = self.track_list.lock().await;
    let new_index = 1 + self.current_index.fetch_add(1, Ordering::Release);

    if new_index >= tracks.len() {
      self.stop_or_wrap_track(&tracks, false).await?;
    } else {
      // Drop tracks guard to prevent deadlock
      mem::drop(tracks);
      self.requeue_current_track().await?;
    }

    Ok(())
  }

  pub async fn go_to_previous_track(&self, soft: bool) -> Result<(), PlayerError> {
    const RESTART_THRESHOLD: Duration = Duration::from_secs(5);

    if soft && self.position().await > RESTART_THRESHOLD {
      self.seek(SeekPosition::To(Duration::ZERO)).await
    } else {
      let current_index = self.current_index.load(Ordering::Acquire);
      let new_index = current_index.saturating_sub(1);
      self.current_index.store(new_index, Ordering::Release);

      if current_index == 0 {
        self.current_index.store(0, Ordering::Release);

        let tracks = self.track_list.lock().await;
        self.stop_or_wrap_track(&tracks, true).await?;
      } else {
        self.requeue_current_track().await?;
      }

      Ok(())
    }
  }

  pub async fn shuffle(&self) -> bool {
    self.shuffle_order.lock().await.is_some()
  }

  pub async fn set_shuffle(&self, shuffle: bool) -> Result<(), PlayerError> {
    let prev_shuffle = self.shuffle_order.lock().await.take().is_some();
    if shuffle != prev_shuffle {
      if shuffle {
        self.shuffle_tracks().await?;
      }

      self.emit(Event::ShuffleChanged(shuffle))?;
      println!("Shuffle set to {shuffle}");
    }

    Ok(())
  }

  pub async fn loop_mode(&self) -> LoopMode {
    self.controls.loop_mode.load(Ordering::Relaxed)
  }

  pub async fn set_loop_mode(&self, loop_mode: LoopMode) -> Result<(), PlayerError> {
    let prev_mode = self.controls.loop_mode.swap(loop_mode, Ordering::Relaxed);
    if loop_mode != prev_mode {
      self.emit(Event::LoopModeChanged(loop_mode))?;
      println!("Loop mode set to {loop_mode:?}");
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
      println!("volume set to {volume:?}");
    }

    Ok(())
  }

  pub async fn position(&self) -> Duration {
    *self.controls.position.lock().await
  }

  pub async fn seek(&self, seek_position: SeekPosition) -> Result<(), PlayerError> {
    if matches!(
      *self.controls.next_source.lock().await,
      NextSourceState::None
    ) {
      return Ok(());
    }

    let (tx, rx) = oneshot::oneshot();
    *self.controls.seek_position.lock().await = Some((seek_position, tx));

    rx.await.map_err(|_| SeekError::ErrorChannelClosed)??;
    println!("Seeked {seek_position:?}");

    Ok(())
  }

  pub async fn current_track(&self) -> Option<Arc<Track>> {
    let tracks = self.track_list.lock().await;
    let track_index = self.get_shuffled_index().await;
    tracks.get(track_index).map(|track| track.clone())
  }

  /// Inserts new tracks at a specified position in the track list
  pub async fn insert_tracks(
    &self,
    position: InsertPosition,
    new_tracks: &[Arc<Track>],
  ) -> Result<(), PlayerError> {
    if matches!(position, InsertPosition::Replace) {
      self.clear_tracks().await?;
    }

    let shuffle = self.shuffle().await;
    let mut tracks = self.track_list.lock().await;

    // If shuffle is enabled, don't do relative insertion and default to end
    let current_index = if !shuffle {
      self.current_index.load(Ordering::Acquire)
    } else {
      tracks.len()
    };

    let mut track_index = position.get_absolute(current_index, tracks.len());

    let current_track_changed = matches!(position, InsertPosition::Relative(0));
    if tracks.len() > 0 && track_index <= current_index && !current_track_changed {
      // If the track was instered before the current one, increment the index to keep the same track playing
      self
        .current_index
        .fetch_add(new_tracks.len(), Ordering::Release);
    }

    for track in new_tracks {
      tracks.insert(track_index, track.clone());
      track_index += 1;
    }

    // Drop tracks lock to prevent deadlock
    mem::drop(tracks);

    if shuffle {
      self.shuffle_tracks().await?;
    }

    if current_track_changed
      && !matches!(
        self.controls.playback_state.load(Ordering::Acquire),
        PlaybackState::Stopped
      )
    {
      self.requeue_current_track().await?;
    }

    Ok(())
  }

  pub async fn clear_tracks(&self) -> Result<(), PlayerError> {
    self.stop().await?;
    *self.track_list.lock().await = Vec::new();
    self.current_index.store(0, Ordering::Release);
    println!("Clearing track list");

    Ok(())
  }

  pub async fn run(&self) -> Result<(), PlayerError> {
    loop {
      let event = self
        .source_rx
        .recv()
        .await
        .map_err(|_| PlayerError::SourceChannelClosed)?;

      if event.indicates_end() {
        if !matches!(event, SourceEvent::Skipped) {
          if let Err(error) = self.go_to_next_track().await {
            if error.is_recoverable() {
              eprintln!("{error}");
            } else {
              return Err(error);
            }
          }
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
