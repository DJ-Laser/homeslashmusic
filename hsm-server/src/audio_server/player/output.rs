use std::{fmt::Debug, mem, sync::Arc, time::Duration};

use rodio::{Sample, Source, source};

use super::Controls;

pub enum SourceQueueState {
  Queued(Box<dyn Source + Send>),
  Playing,
  None,
}

impl Debug for SourceQueueState {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Queued(_) => write!(f, "Queued(Box<dyn Source>)"),
      Self::Playing => write!(f, "Playing"),
      Self::None => write!(f, "None"),
    }
  }
}

impl SourceQueueState {
  pub fn is_queued(&self) -> bool {
    return matches!(self, Self::Queued(_));
  }

  pub fn is_playing(&self) -> bool {
    return !matches!(self, Self::None);
  }

  pub fn invalidate(&mut self) {
    match self {
      Self::Queued(_) => *self = Self::Playing,
      Self::Playing | Self::None => (),
    }
  }

  pub fn consume(&mut self) -> Option<Box<dyn Source + Send>> {
    match self {
      Self::Queued(_) => {
        let state = mem::replace(self, Self::Playing);
        let Self::Queued(source) = state else {
          unreachable!("Moved out of a SourceQueueState::Queued")
        };

        Some(source)
      }
      Self::Playing => {
        *self = Self::None;
        None
      }
      Self::None => None,
    }
  }
}

pub struct PlayerAudioOutput {
  current: Box<dyn Source + Send>,
  controls: Arc<Controls>,
}

impl PlayerAudioOutput {
  const THRESHOLD: usize = 512;

  pub(super) fn new(controls: Arc<Controls>) -> Self {
    Self {
      current: Box::new(source::Empty::new()) as Box<_>,
      controls,
    }
  }

  fn load_next(&mut self) {
    self.current = {
      let mut next = self.controls.source_queue.lock_blocking();

      match next.consume() {
        Some(next) => next,
        None => Box::new(source::Zero::new_samples(1, 44100, Self::THRESHOLD)) as Box<_>,
      }
    }
  }
}

impl Iterator for PlayerAudioOutput {
  type Item = Sample;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    loop {
      if let Some(sample) = self.current.next() {
        return Some(sample);
      }

      self.load_next();
    }
  }

  #[inline]
  fn size_hint(&self) -> (usize, Option<usize>) {
    (self.current.size_hint().0, None)
  }
}

impl Source for PlayerAudioOutput {
  #[inline]
  fn current_span_len(&self) -> Option<usize> {
    // This function is non-trivial because the boundary between two sounds in the queue should
    // be a span boundary as well.
    //
    // The current sound is free to return `None` for `current_span_len()`, in which case
    // we *should* return the number of samples remaining the current sound.
    // This can be estimated with `size_hint()`.
    //
    // If the `size_hint` is `None` as well, we are in the worst case scenario. To handle this
    // situation we force a span to have a maximum number of samples indicate by this
    // constant.

    // Try the current `current_span_len`.
    if let Some(val) = self.current.current_span_len() {
      if val != 0 {
        return Some(val);
      } else {
        // The next source will be a filler silence which will have the length of `THRESHOLD`
        return Some(Self::THRESHOLD);
      }
    }

    // Try the size hint.
    let (lower_bound, _) = self.current.size_hint();
    // The iterator default implementation just returns 0.
    // That's a problematic value, so skip it.
    if lower_bound > 0 {
      return Some(lower_bound);
    }

    // Otherwise we use the constant value.
    Some(Self::THRESHOLD)
  }

  #[inline]
  fn channels(&self) -> rodio::ChannelCount {
    self.current.channels()
  }

  #[inline]
  fn sample_rate(&self) -> rodio::SampleRate {
    self.current.sample_rate()
  }

  #[inline]
  fn total_duration(&self) -> Option<Duration> {
    None
  }

  #[inline]
  fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
    self.current.try_seek(pos)
  }
}
