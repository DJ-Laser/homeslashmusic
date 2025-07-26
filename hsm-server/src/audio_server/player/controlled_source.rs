use std::{
  sync::{Arc, atomic::Ordering},
  time::Duration,
};

use hsm_ipc::SeekPosition;
use rodio::{
  Source,
  source::{Amplify, Pausable, SeekError, Skippable, TrackPosition},
};
use smol::channel::Sender;

use super::{Controls, LoopMode, PlaybackState};

pub enum SourceEvent {
  Seeked(Duration),
  LoopError(SeekError),
  Finished,
  Looped,
}

impl SourceEvent {
  /// If this event indicates that the soutrce has ended.
  /// Used to manage the player's internal source count
  pub fn indicates_end(&self) -> bool {
    match self {
      Self::Finished | Self::LoopError(_) => true,
      _ => false,
    }
  }
}

type WrappedSourceInner<S> = ControlledSource<Skippable<Pausable<Amplify<TrackPosition<S>>>>>;

const SOURCE_UPDATE_INTERVAL: Duration = Duration::from_millis(5);

pub struct ControlledSource<I> {
  input: I,
  controls: Arc<Controls>,
  source_tx: Sender<SourceEvent>,
}

impl<I> ControlledSource<I>
where
  I: Source,
{
  #[inline]
  pub fn with_controls(&mut self, f: impl FnOnce(&mut I, &Arc<Controls>, &Sender<SourceEvent>)) {
    f(&mut self.input, &self.controls, &self.source_tx)
  }
}

impl<I> Iterator for ControlledSource<I>
where
  I: Source,
{
  type Item = I::Item;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if let Some(value) = self.input.next() {
      return Some(value);
    }

    if matches!(
      self.controls.loop_mode.load(Ordering::Relaxed),
      LoopMode::Track,
    ) {
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
}

fn control_wrapped_source<S: Source>(controlled: &mut WrappedSourceInner<S>) {
  controlled.with_controls(|skippable, controls, source_tx| {
    let to_skip = controls.to_skip.load(Ordering::Acquire);
    if to_skip > 0 {
      skippable.skip();
      controls.to_skip.store(to_skip - 1, Ordering::Release);
      return;
    }

    let pauseable = skippable.inner_mut();
    pauseable.set_paused(!matches!(
      controls.playback_state.load(Ordering::Relaxed),
      PlaybackState::Playing
    ));

    let volume_controlled = pauseable.inner_mut();
    volume_controlled.set_factor(*controls.volume.lock_blocking());

    let position_tracked = volume_controlled.inner_mut();
    if let Some((seek_position, mut tx)) = controls.seek_position.lock_blocking().take() {
      let current_position = position_tracked.get_pos();
      let seek_position = match seek_position {
        SeekPosition::Forward(duration) => current_position.saturating_add(duration),
        SeekPosition::Backward(duration) => current_position.saturating_sub(duration),
        SeekPosition::To(position) => position,
      };

      let _ = tx.send(
        position_tracked
          .try_seek(seek_position)
          .map_err(|error| super::errors::SeekError::SeekFailed(error.to_string())),
      );

      let _ = source_tx.try_send(SourceEvent::Seeked(seek_position));
    }

    *controls.position.lock_blocking() = position_tracked.get_pos();
  });
}

pub fn wrap_source<S: Source>(
  source: S,
  controls: Arc<Controls>,
  source_tx: Sender<SourceEvent>,
) -> impl Source {
  let wrapped = source
    .track_position()
    .amplify(1.0)
    .pausable(false)
    .skippable();

  let controlled = ControlledSource {
    input: wrapped,
    controls,
    source_tx,
  };

  controlled.periodic_access(SOURCE_UPDATE_INTERVAL, control_wrapped_source)
}
