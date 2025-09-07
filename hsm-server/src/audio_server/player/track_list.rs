use std::{
  ops::Index,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
};

use hsm_ipc::{InsertPosition, Track};
use rand::seq::SliceRandom;
use smol::lock::Mutex;

use super::PlayerError;

/// `track_list` and `shuffle_order` must always have the same `len()``
struct TrackListInner {
  track_list: Vec<Arc<Track>>,
  shuffled_track_indicies: Vec<usize>,
}

impl TrackListInner {
  pub fn new() -> Self {
    Self {
      track_list: Vec::new(),
      shuffled_track_indicies: Vec::new(),
    }
  }

  pub fn clear(&mut self) {
    self.track_list.clear();
    self.shuffled_track_indicies.clear();
  }

  pub fn len(&self) -> usize {
    debug_assert_eq!(self.track_list.len(), self.track_list.len());
    self.track_list.len()
  }

  pub fn insert_tracks(&mut self, index: usize, tracks: &[Arc<Track>]) {
    self.track_list.splice(index..index, tracks.iter().cloned());

    // Update shuffle indicies to point to the updated track positions
    for shuffle_index in self.shuffled_track_indicies.iter_mut() {
      if *shuffle_index >= index {
        *shuffle_index += tracks.len();
      }
    }

    // Add shuffle indicies corresponding to the inserted tracks (1:1 index mapping)
    self
      .shuffled_track_indicies
      .splice(index..index, index..index + tracks.len());
  }
}

impl Index<usize> for TrackListInner {
  type Output = Arc<Track>;

  fn index(&self, index: usize) -> &Self::Output {
    debug_assert_eq!(self.track_list.len(), self.track_list.len());
    &self.track_list[self.shuffled_track_indicies[index]]
  }
}

/// Manages the track list and index.
///
/// To reduce the need for locking, relevant data is stored in atomics insteadd of locking the track list
pub struct TrackList {
  inner: Mutex<TrackListInner>,
  track_list_len: AtomicUsize,
  shuffle_enabled: AtomicBool,
}

impl TrackList {
  pub fn new() -> Self {
    Self {
      inner: Mutex::new(TrackListInner::new()),
      track_list_len: AtomicUsize::new(0),
      shuffle_enabled: AtomicBool::new(false),
    }
  }

  pub fn len(&self) -> usize {
    self.track_list_len.load(Ordering::Acquire)
  }

  pub async fn get_track(&self, index: usize) -> Option<Arc<Track>> {
    let num_tracks = self.track_list_len.load(Ordering::Acquire);

    if index >= num_tracks {
      return None;
    }

    let inner = self.inner.lock().await;
    Some(inner[index].clone())
  }

  /// Returns `None` if the track list length is zero
  pub async fn get_tracks_to_queue(
    &self,
    index: usize,
  ) -> Option<(Arc<Track>, Option<Arc<Track>>)> {
    let num_tracks = self.track_list_len.load(Ordering::Acquire);

    if index >= num_tracks {
      return None;
    }

    let inner = self.inner.lock().await;

    let current_track = inner[index].clone();

    let is_last_track = index == num_tracks - 1;
    let next_track = if !is_last_track {
      Some(inner[index + 1].clone())
    } else {
      None
    };

    Some((current_track, next_track))
  }

  async fn shuffle_tracks(&self) -> Result<(), PlayerError> {
    let num_tracks = self.track_list_len.load(Ordering::Acquire);

    if num_tracks == 0 {
      return Ok(());
    }

    let mut inner = self.inner.lock().await;
    inner.shuffled_track_indicies.shuffle(&mut rand::rng());

    Ok(())
  }

  pub fn shuffle_enabled(&self) -> bool {
    self.shuffle_enabled.load(Ordering::Acquire)
  }

  pub async fn set_shuffle(&self, shuffle: bool) -> Result<(), PlayerError> {
    if shuffle {
      self.shuffle_tracks().await?;
    } else {
    }

    self.shuffle_enabled.store(shuffle, Ordering::Release);

    Ok(())
  }

  pub async fn clear(&self) -> Result<(), PlayerError> {
    let mut inner = self.inner.lock().await;
    inner.clear();
    self.track_list_len.store(0, Ordering::Release);

    Ok(())
  }

  /// Returns the new position of `current_index`
  pub async fn insert_tracks(
    &self,
    current_index: usize,
    position: InsertPosition,
    tracks: &[Arc<Track>],
  ) -> Result<usize, PlayerError> {
    let mut inner = self.inner.lock().await;

    let insert_index = position.get_absolute(current_index, inner.len());
    let mut new_current_index = current_index;

    if matches!(position, InsertPosition::Replace) {
      inner.clear();
    }

    if inner.len() > 0 && insert_index <= current_index {
      // If the tracks were instered before the current one, increment the index to keep it referencing the current track
      new_current_index += tracks.len()
    }

    inner.insert_tracks(insert_index, tracks);

    // Move new shuffle indicies to random locations
    if self.shuffle_enabled() {
      //TODO: Move to  positions and update `new_current_index`
      /*let inserted_track_range = insert_index..(insert_index + tracks.len());
      let shuffle_indicies: Vec<usize> = inner
        .shuffled_track_indicies
        .drain(inserted_track_range)
        .collect();

      for shuffle_index in shuffle_indicies {
      }*/
    }

    self.track_list_len.store(inner.len(), Ordering::Release);

    Ok(new_current_index)
  }
}
