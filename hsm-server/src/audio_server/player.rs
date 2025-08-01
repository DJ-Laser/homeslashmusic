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

pub enum InsertPosition {
  Absolute(usize),
  /// relative to current: `0` for current, `1` for next, etc
  Relative(isize),
  Start,
  End,
}

impl InsertPosition {
  fn get_absolute(&self, current_position: usize, track_list_len: usize) -> usize {
    let position = match self {
      InsertPosition::Absolute(position) => *position,
      InsertPosition::Relative(delta) => {
        if delta.is_negative() {
          current_position.saturating_sub(delta.abs() as usize)
        } else {
          current_position.saturating_add(delta.abs() as usize)
        }
      }
      InsertPosition::Start => 0,
      InsertPosition::End => track_list_len,
    };

    position.clamp(0, track_list_len)
  }
}

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
  track_list: Mutex<Vec<Arc<Track>>>,
  current_index: AtomicUsize,

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
        track_list: Mutex::new(Vec::new()),
        current_index: AtomicUsize::new(0),
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
    if matches!(
      self.controls.playback_state.load(Ordering::Acquire),
      PlaybackState::Stopped
    ) {
      let had_tracks = match self.recreate_source_queue().await {
        Ok(had_tracks) => had_tracks,
        Err(error) => {
          eprintln!("Failed to resume playback: {error}");
          return Ok(());
        }
      };

      if !had_tracks {
        println!("Not playing, stopped");
        return Ok(());
      }
    }

    self.set_playback_state(PlaybackState::Playing)?;
    println!("PLAYING");

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
    match current_state {
      PlaybackState::Paused | PlaybackState::Stopped => self.play().await?,
      PlaybackState::Playing => self.pause().await?,
    }

    Ok(())
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

  async fn clear_source_queue(&self) {
    self.skip(self.source_count.load(Ordering::Acquire));
  }

  async fn add_track_to_queue(&self, track: &Arc<Track>) -> Result<(), LoadTrackError> {
    let source = self.wrap_source(TrackDecoder::new(track.as_ref().clone()).await?);
    self.source_queue.append(source);
    self.source_count.fetch_add(1, Ordering::Release);

    Ok(())
  }

  async fn preload_next_track(&self) -> Result<(), LoadTrackError> {
    let tracks = self.track_list.lock().await;
    let current_index = self.current_index.load(Ordering::Acquire);
    if let Some(next_track) = tracks.get(current_index + 1) {
      self.add_track_to_queue(&next_track).await?;
    }

    Ok(())
  }

  /// Returns false if there were no tracks to load
  async fn recreate_source_queue(&self) -> Result<bool, LoadTrackError> {
    self.clear_source_queue().await;
    {
      let tracks = self.track_list.lock().await;
      if tracks.len() == 0 {
        return Ok(false);
      }

      let current_index = self.current_index.load(Ordering::Acquire);
      let current_track = &tracks[current_index];
      self.add_track_to_queue(&current_track).await?;
    }

    // Drop tracks to prevent deadlock
    self.preload_next_track().await?;

    Ok(true)
  }

  /// Inserts a new track at a specified position in the track list
  pub async fn insert_track(
    &self,
    position: InsertPosition,
    track: Arc<Track>,
  ) -> Result<(), LoadTrackError> {
    {
      let mut tracks = self.track_list.lock().await;
      let current_index = self.current_index.load(Ordering::Acquire);
      let track_index = position.get_absolute(current_index, tracks.len());

      if tracks.len() > 0 && track_index <= current_index {
        // If the track was instered before the current one, increment the index to keep the same track playing
        self.current_index.fetch_add(1, Ordering::Release);
      }

      tracks.insert(track_index, track);
      println!("idx: {track_index}");
      println!("Len: {}", tracks.len());
    }

    // Drop tracks to prevent deadlock
    if !matches!(
      self.controls.playback_state.load(Ordering::Acquire),
      PlaybackState::Stopped
    ) {
      self.recreate_source_queue().await?;
    }

    Ok(())
  }

  pub async fn current_track(&self) -> Option<Arc<Track>> {
    let tracks = self.track_list.lock().await;
    let current_index = self.current_index.load(Ordering::Acquire);
    tracks.get(current_index).map(|track| track.clone())
  }

  pub async fn increment_current_index(&self) -> Result<Result<(), LoadTrackError>, PlayerError> {
    {
      let tracks = self.track_list.lock().await;
      let new_index = 1 + self.current_index.fetch_add(1, Ordering::Release);
      println!("finished, incremented to: {new_index}");
      println!("len: {}", tracks.len());

      if new_index >= tracks.len() {
        println!("reached end, index=0");
        self.current_index.store(0, Ordering::Release);

        if tracks.len() == 0
          || !matches!(
            self.controls.loop_mode.load(Ordering::Acquire),
            LoopMode::Playlist
          )
        {
          println!("Stopping");
          self.stop().await?;
        } else {
          println!("Looping");
          if let Err(error) = self.add_track_to_queue(&tracks[0]).await {
            return Ok(Err(error));
          };
        }
      }
    }

    // Drop tracks to prevent deadlock
    Ok(self.preload_next_track().await)
  }

  pub async fn run(&self) -> Result<(), PlayerError> {
    loop {
      let event = self
        .source_rx
        .recv()
        .await
        .map_err(|_| PlayerError::SourceChannelClosed)?;

      if event.indicates_end() {
        self.source_count.fetch_sub(1, Ordering::Acquire);
        if !matches!(event, SourceEvent::Skipped) {
          self
            .increment_current_index()
            .await?
            .unwrap_or_else(|error| eprintln!("Failed to resume load next track: {error}"));
        }

        // Set state to stopped on last source finish
        if self.source_count.load(Ordering::Acquire) == 0 {
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
