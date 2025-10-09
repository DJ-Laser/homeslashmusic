use std::{sync::Arc, time::Duration};

use async_oneshot as oneshot;
use hsm_ipc::SeekPosition;
use rodio::{
  Source,
  source::{Amplify, Pausable, SeekError as RodioSeekError, TrackPosition},
};
use smol::{
  channel::Sender,
  lock::{Mutex, MutexGuard},
};
use thiserror::Error;

use super::{LoopMode, PlaybackState};

type SeekResult = Result<(), SeekError>;

#[derive(Debug)]
struct ControlsInner {
  pub playback_state: PlaybackState,
  pub loop_mode: LoopMode,
  pub volume: f32,
  pub to_skip: usize,
  pub position: Duration,
  pub seek_position: Option<(SeekPosition, oneshot::Sender<SeekResult>)>,
}

#[derive(Debug)]
pub struct Controls(Mutex<ControlsInner>);

impl Controls {
  pub fn new() -> Self {
    Self(Mutex::new(ControlsInner {
      playback_state: PlaybackState::Stopped,
      loop_mode: LoopMode::None,
      to_skip: 0,
      volume: 1.0,
      position: Duration::ZERO,
      seek_position: None,
    }))
  }

  async fn lock(&self) -> MutexGuard<ControlsInner> {
    self.0.lock().await
  }

  fn lock_blocking(&self) -> MutexGuard<ControlsInner> {
    self.0.lock_blocking()
  }

  pub async fn playback_state(&self) -> PlaybackState {
    self.lock().await.playback_state
  }

  pub async fn set_playback_state(&self, playback_state: PlaybackState) -> PlaybackState {
    std::mem::replace(&mut self.lock().await.playback_state, playback_state)
  }

  pub async fn loop_mode(&self) -> LoopMode {
    self.lock().await.loop_mode
  }

  pub async fn set_loop_mode(&self, loop_mode: LoopMode) -> LoopMode {
    std::mem::replace(&mut self.lock().await.loop_mode, loop_mode)
  }

  pub async fn skip_one(&self) {
    self.lock().await.to_skip += 1;
  }

  pub async fn volume(&self) -> f32 {
    self.lock().await.volume
  }

  pub async fn set_volume(&self, volume: f32) -> f32 {
    std::mem::replace(&mut self.lock().await.volume, volume)
  }

  pub async fn position(&self) -> Duration {
    self.lock().await.position
  }

  pub async fn reset_position(&self) {
    self.lock().await.position = Duration::ZERO;
  }

  pub async fn seek(&self, seek_position: SeekPosition) -> oneshot::Receiver<SeekResult> {
    let (tx, rx) = oneshot::oneshot();
    self.lock().await.seek_position = Some((seek_position, tx));
    rx
  }
}

pub enum SourceEvent {
  Seeked(Duration),
  LoopError(RodioSeekError),
  Finished,
  Skipped,
  Looped,
}

impl SourceEvent {
  /// If this event indicates that the soutrce has ended.
  /// Used to manage the player's internal source count
  pub fn indicates_end(&self) -> bool {
    matches!(self, Self::Finished | Self::LoopError(_))
  }
}

type ControlledSourceInner<S> = Pausable<Amplify<TrackPosition<S>>>;

pub const SOURCE_UPDATE_INTERVAL: Duration = Duration::from_millis(5);

struct ControlledSource<I> {
  input: ControlledSourceInner<I>,
  controls: Arc<Controls>,
  source_tx: Sender<SourceEvent>,

  should_skip: bool,
  cached_loop_mode: LoopMode,
}

impl<I> ControlledSource<I>
where
  I: Source,
{
  pub fn new(input: I, controls: Arc<Controls>, source_tx: Sender<SourceEvent>) -> Self {
    Self {
      input: input.track_position().amplify(1.0).pausable(false),
      controls,
      source_tx,

      should_skip: false,
      cached_loop_mode: LoopMode::None,
    }
  }

  fn update_control_state(&mut self) {
    let mut controls = self.controls.lock_blocking();

    self.cached_loop_mode = controls.loop_mode;
    if controls.to_skip > 0 {
      self.should_skip = true;
      controls.to_skip -= 1;
      return;
    }

    let pausable = &mut self.input;
    pausable.set_paused(!matches!(controls.playback_state, PlaybackState::Playing));

    let volume_controlled = pausable.inner_mut();
    volume_controlled.set_factor(controls.volume);

    let position_tracked = volume_controlled.inner_mut();
    if let Some((seek_position, mut tx)) = controls.seek_position.take() {
      let current_position = position_tracked.get_pos();
      let seek_position = match seek_position {
        SeekPosition::Forward(duration) => current_position.saturating_add(duration),
        SeekPosition::Backward(duration) => current_position.saturating_sub(duration),
        SeekPosition::To(position) => position,
      };

      let _ = tx.send(
        position_tracked
          .try_seek(seek_position)
          .map_err(|error| SeekError::SeekFailed(error.to_string())),
      );

      let _ = self.source_tx.try_send(SourceEvent::Seeked(seek_position));
    }

    controls.position = position_tracked.get_pos();
  }
}

impl<I> Iterator for ControlledSource<I>
where
  I: Source,
{
  type Item = I::Item;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.should_skip {
      let _ = self.source_tx.try_send(SourceEvent::Skipped);
      return None;
    }

    if let Some(value) = self.input.next() {
      return Some(value);
    }

    if matches!(self.cached_loop_mode, LoopMode::Track) {
      if let Err(error) = self.input.try_seek(Duration::ZERO) {
        let _ = self.source_tx.try_send(SourceEvent::LoopError(error));
        return None;
      }

      let _ = self.source_tx.try_send(SourceEvent::Looped);
      self.input.next()
    } else {
      let _ = self.source_tx.try_send(SourceEvent::Finished);
      None
    }
  }

  #[inline]
  fn size_hint(&self) -> (usize, Option<usize>) {
    self.input.size_hint()
  }
}

#[derive(Debug, Error)]
pub enum SeekError {
  #[error("Internal Player Error: SeekError channel closed")]
  ErrorChannelClosed,

  #[error("{0}")]
  SeekFailed(String),
}

impl<I> Source for ControlledSource<I>
where
  I: Source,
{
  #[inline]
  fn current_span_len(&self) -> Option<usize> {
    self.input.current_span_len()
  }

  #[inline]
  fn channels(&self) -> rodio::ChannelCount {
    self.input.channels()
  }

  #[inline]
  fn sample_rate(&self) -> rodio::SampleRate {
    self.input.sample_rate()
  }

  #[inline]
  fn total_duration(&self) -> Option<Duration> {
    self.input.total_duration()
  }

  fn try_seek(&mut self, pos: Duration) -> Result<(), RodioSeekError> {
    self.input.try_seek(pos)
  }
}

pub fn wrap_source<S: Source>(
  source: S,
  controls: Arc<Controls>,
  source_tx: Sender<SourceEvent>,
) -> impl Source {
  ControlledSource::new(source, controls, source_tx).periodic_access(
    SOURCE_UPDATE_INTERVAL,
    ControlledSource::update_control_state,
  )
}
