use std::{
  sync::{Arc, atomic::Ordering},
  time::Duration,
};

use hsm_ipc::SeekPosition;
use rodio::{
  Source,
  source::{Amplify, Pausable, SeekError as RodioSeekError, TrackPosition},
};
use smol::channel::Sender;
use thiserror::Error;

use super::{Controls, LoopMode, PlaybackState, output::SourceQueueState};

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
    }
  }

  fn update_control_state(&mut self) {
    let to_skip = self.controls.to_skip.load(Ordering::Acquire);
    if to_skip > 0 {
      self.should_skip = true;
      self.controls.to_skip.fetch_sub(1, Ordering::Release);
      return;
    }

    let pausable = &mut self.input;
    pausable.set_paused(!matches!(
      self.controls.playback_state.load(Ordering::Relaxed),
      PlaybackState::Playing
    ));

    let volume_controlled = pausable.inner_mut();
    volume_controlled.set_factor(*self.controls.volume.lock_blocking());

    let position_tracked = volume_controlled.inner_mut();
    if let Some((seek_position, mut tx)) = self.controls.seek_position.lock_blocking().take() {
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

    *self.controls.position.lock_blocking() = position_tracked.get_pos();
  }

  fn clear_playing_source(&self) {
    let mut next_source = self.controls.source_queue.lock_blocking();
    if matches!(*next_source, SourceQueueState::Playing) {
      *next_source = SourceQueueState::None;
    }
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
      self.clear_playing_source();
      return None;
    }

    if let Some(value) = self.input.next() {
      return Some(value);
    }

    if matches!(
      self.controls.loop_mode.load(Ordering::Relaxed),
      LoopMode::Track,
    ) {
      if let Err(error) = self.input.try_seek(Duration::ZERO) {
        let _ = self.source_tx.try_send(SourceEvent::LoopError(error));
        self.clear_playing_source();
        return None;
      }

      let _ = self.source_tx.try_send(SourceEvent::Looped);
      self.input.next()
    } else {
      let _ = self.source_tx.try_send(SourceEvent::Finished);
      self.clear_playing_source();
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
