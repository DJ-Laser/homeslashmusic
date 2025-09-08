use std::{
  ops::Index,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
};

use hsm_ipc::{InsertPosition, Track};
use rand::{Rng, seq::SliceRandom};
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

  /// Inserts tracks into the `track_list`
  /// Does not insert shuffle indicies, instead returns an iterator of shuffle indicies to insert
  /// These indicies must be added into `shuffled_track_indicies`` before calling any other method
  pub fn insert_tracks(
    &mut self,
    index: usize,
    tracks: &[Arc<Track>],
  ) -> impl Iterator<Item = usize> {
    debug_assert_eq!(self.track_list.len(), self.track_list.len());

    self.track_list.splice(index..index, tracks.iter().cloned());

    // Update shuffle indicies to point to the updated track positions
    for shuffle_index in self.shuffled_track_indicies.iter_mut() {
      if *shuffle_index >= index {
        *shuffle_index += tracks.len();
      }
    }

    // return shuffle indicies corresponding to the inserted tracks
    index..index + tracks.len()
  }

  fn shuffle_tracks(&mut self, rng: &mut impl Rng) {
    self.shuffled_track_indicies.shuffle(rng);
  }

  fn order_tracks(&mut self) {
    self.shuffled_track_indicies.clear();
    self
      .shuffled_track_indicies
      .extend(0..self.track_list.len());
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

  pub fn shuffle_enabled(&self) -> bool {
    self.shuffle_enabled.load(Ordering::Acquire)
  }

  pub async fn set_shuffle(&self, shuffle: bool) -> Result<(), PlayerError> {
    let mut inner = self.inner.lock().await;

    if shuffle {
      inner.shuffle_tracks(&mut rand::rng());
    } else {
      inner.order_tracks();
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

    if matches!(position, InsertPosition::Replace) {
      inner.clear();
    }

    let track_list_started_empty = inner.len() == 0;

    // Insert `Next` tracks into the tracks list after the current song, even if it has been shuffled
    let track_index = if !track_list_started_empty {
      inner.shuffled_track_indicies[current_index]
    } else {
      0
    };

    let insert_index = position.get_absolute(track_index, inner.len());

    let shuffle_indicies: Vec<usize> = inner.insert_tracks(insert_index, tracks).collect();
    let shuffled_track_indicies = &mut inner.shuffled_track_indicies;

    let mut new_current_index = current_index;
    if self.shuffle_enabled.load(Ordering::Acquire) {
      let mut rng = rand::rng();

      // Move new shuffle indicies to random locations
      for shuffle_index in shuffle_indicies {
        let new_index = rng.random_range(0..shuffled_track_indicies.len());

        shuffled_track_indicies.insert(new_index, shuffle_index);
        if new_index <= new_current_index {
          new_current_index += 1;
        }
      }
    } else {
      if insert_index <= new_current_index {
        new_current_index += shuffle_indicies.len();
      }

      shuffled_track_indicies.splice(insert_index..insert_index, shuffle_indicies);
    }

    self.track_list_len.store(inner.len(), Ordering::Release);

    if !track_list_started_empty {
      Ok(new_current_index)
    } else {
      Ok(0)
    }
  }
}
