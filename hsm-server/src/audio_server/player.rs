use std::{
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
use rodio::{Source, mixer::Mixer};
use smol::{
  channel::{self, Receiver, Sender},
  lock::Mutex,
};

use atomic_control_status::{AtomicLoopMode, AtomicPlaybackState};
use thiserror::Error;
use track_list::TrackList;

use super::{event::Event, track::LoadTrackError};
pub use output::PlayerAudioOutput;

mod atomic_control_status;
mod controlled_source;
mod decoder;
mod output;
mod track_list;

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

  #[error("failed to seek: ")]
  SeekFailed(#[from] SeekError),
}

impl PlayerError {
  pub fn is_recoverable(&self) -> bool {
    match self {
      Self::LoadTrack(_) => true,
      Self::SeekFailed(_) => true,
      _ => false,
    }
  }
}

pub struct Player {
  tracks: TrackList,

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
      tracks: TrackList::new(),

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

  /// Returns true if there was a current track to queue
  async fn requeue_current_track(&self) -> Result<bool, LoadTrackError> {
    self.clear_source_queue().await;
    let Some((current_track, next_track)) = self.tracks.get_current_and_next_track().await else {
      return Ok(false);
    };

    self.queue_track(&current_track).await?;
    if let Some(next_track) = next_track {
      self.queue_track(&next_track).await?
    }

    Ok(true)
  }

  pub fn playback_state(&self) -> PlaybackState {
    self.controls.playback_state.load(Ordering::Acquire)
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

  async fn stop_or_wrap_track(&self, reverse: bool) -> Result<(), PlayerError> {
    /*let printed_position = if reverse { "beginning" } else { "end" };
    let printed_loop_position = if reverse { "end" } else { "beginning" };

    let tracks = self.track_list.lock().await;
    let should_loop = !matches!(
      self.controls.loop_mode.load(Ordering::Acquire),
      LoopMode::None
    );

    // Don't skip to end if loop is off
    let new_index = if should_loop && reverse {
      tracks.len() - 1
    } else {
      0
    };

    self.current_index.store(new_index, Ordering::Release);

    if !should_loop || tracks.len() == 0 {
      println!("Track list reached {printed_position}, stopping");
      self.stop().await?;
    } else {
      println!("Track list reached {printed_position}, looping to {printed_loop_position}");

      // Drop tracks guard to prevent deadlock
      mem::drop(tracks);
      self.requeue_current_track().await?;
    };*/

    Ok(())
  }

  pub async fn go_to_next_track(&self) -> Result<(), PlayerError> {
    /*let tracks = self.track_list.lock().await;
    let new_index = 1 + self.current_index.fetch_add(1, Ordering::Release);
    let wrap_needed = new_index >= tracks.len();

    // Drop tracks guard to prevent deadlock
    mem::drop(tracks);

    if wrap_needed {
      self.stop_or_wrap_track(false).await?;
    } else {
      self.requeue_current_track().await?;
    }*/

    Ok(())
  }

  pub async fn go_to_previous_track(&self, soft: bool) -> Result<(), PlayerError> {
    /*const RESTART_THRESHOLD: Duration = Duration::from_secs(5);

    if soft && self.position().await > RESTART_THRESHOLD {
      self.seek(SeekPosition::To(Duration::ZERO)).await
    } else {
      let current_index = self.current_index.load(Ordering::Acquire);

      if current_index == 0 {
        self.stop_or_wrap_track(true).await?;
      } else {
        self
          .current_index
          .store(current_index - 1, Ordering::Release);
        self.requeue_current_track().await?;
      }*/

    Ok(())
    //}
  }

  pub async fn shuffle(&self) -> bool {
    self.tracks.shuffle_enabled()
  }

  pub async fn set_shuffle(&self, shuffle: bool) -> Result<(), PlayerError> {
    let prev_shuffle = self.shuffle().await;
    if shuffle != prev_shuffle {
      self.tracks.set_shuffle(shuffle).await?;

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
    self
      .tracks
      .get_current_and_next_track()
      .await
      .map(|tracks| tracks.0)
  }

  pub async fn clear_tracks(&self) -> Result<(), PlayerError> {
    self.stop().await?;
    self.tracks.clear().await?;
    println!("Clearing track list");

    Ok(())
  }

  /// Inserts new tracks at a specified position in the track list
  pub async fn insert_tracks(
    &self,
    position: InsertPosition,
    tracks: &[Arc<Track>],
  ) -> Result<(), PlayerError> {
    self.tracks.insert_tracks(position, tracks).await?;

    // If the track list was replaced, a new song must begin playing
    if matches!(position, InsertPosition::Replace)
      && !matches!(self.playback_state(), PlaybackState::Stopped)
    {
      self.requeue_current_track().await?;
    }

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
