use std::sync::atomic::{AtomicUsize, Ordering};

use hsm_ipc::{LoopMode, PlaybackState};

pub struct AtomicPlaybackState(AtomicUsize);

impl AtomicPlaybackState {
  fn from_usize(val: usize) -> PlaybackState {
    #![allow(non_upper_case_globals)]
    const PLAYING: usize = PlaybackState::Playing as usize;
    const PAUSED: usize = PlaybackState::Paused as usize;
    const STOPPED: usize = PlaybackState::Stopped as usize;
    match val {
      PLAYING => PlaybackState::Playing,
      PAUSED => PlaybackState::Paused,
      STOPPED => PlaybackState::Stopped,
      _ => {
        unreachable!("Invalid enum discriminant")
      }
    }
  }

  pub const fn new(v: PlaybackState) -> Self {
    Self(AtomicUsize::new(v as usize))
  }

  pub fn load(&self, order: Ordering) -> PlaybackState {
    Self::from_usize(self.0.load(order))
  }

  pub fn store(&self, val: PlaybackState, order: Ordering) {
    self.0.store(val as usize, order)
  }

  pub fn swap(&self, val: PlaybackState, order: Ordering) -> PlaybackState {
    Self::from_usize(self.0.swap(val as usize, order))
  }

  pub fn compare_exchange(
    &self,
    current: PlaybackState,
    new: PlaybackState,
    success: Ordering,
    failure: Ordering,
  ) -> Result<PlaybackState, PlaybackState> {
    self
      .0
      .compare_exchange(current as usize, new as usize, success, failure)
      .map(Self::from_usize)
      .map_err(Self::from_usize)
  }
}

pub struct AtomicLoopMode(AtomicUsize);

#[allow(dead_code)]
impl AtomicLoopMode {
  fn from_usize(val: usize) -> LoopMode {
    #![allow(non_upper_case_globals)]
    const NONE: usize = LoopMode::None as usize;
    const TRACK: usize = LoopMode::Track as usize;
    const PLAYLIST: usize = LoopMode::Playlist as usize;
    match val {
      NONE => LoopMode::None,
      TRACK => LoopMode::Track,
      PLAYLIST => LoopMode::Playlist,
      _ => {
        unreachable!("Invalid enum discriminant")
      }
    }
  }

  pub const fn new(v: LoopMode) -> Self {
    Self(AtomicUsize::new(v as usize))
  }

  pub fn load(&self, order: Ordering) -> LoopMode {
    Self::from_usize(self.0.load(order))
  }

  pub fn store(&self, val: LoopMode, order: Ordering) {
    self.0.store(val as usize, order)
  }

  pub fn swap(&self, val: LoopMode, order: Ordering) -> LoopMode {
    Self::from_usize(self.0.swap(val as usize, order))
  }

  pub fn compare_exchange(
    &self,
    current: LoopMode,
    new: LoopMode,
    success: Ordering,
    failure: Ordering,
  ) -> Result<LoopMode, LoopMode> {
    self
      .0
      .compare_exchange(current as usize, new as usize, success, failure)
      .map(Self::from_usize)
      .map_err(Self::from_usize)
  }
}
