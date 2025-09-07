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
  current_track_index: AtomicUsize,
}

impl TrackList {
  pub fn new() -> Self {
    Self {
      inner: Mutex::new(TrackListInner::new()),
      track_list_len: AtomicUsize::new(0),
      shuffle_enabled: AtomicBool::new(false),
      current_track_index: AtomicUsize::new(0),
    }
  }

  /// Returns `None` if the track list length is zero
  pub async fn get_current_and_next_track(&self) -> Option<(Arc<Track>, Option<Arc<Track>>)> {
    let num_tracks = self.track_list_len.load(Ordering::Acquire);

    if num_tracks == 0 {
      return None;
    }

    let inner = self.inner.lock().await;

    let current_index = self.current_track_index.load(Ordering::Acquire);
    let current_track = inner[current_index].clone();

    let is_last_track = current_index == num_tracks - 1;
    let next_track = if !is_last_track {
      Some(inner[current_index + 1].clone())
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
    self.current_track_index.store(0, Ordering::Release);

    Ok(())
  }

  pub async fn insert_tracks(
    &self,
    position: InsertPosition,
    tracks: &[Arc<Track>],
  ) -> Result<(), PlayerError> {
    let mut inner = self.inner.lock().await;

    let current_index = self.current_track_index.load(Ordering::Acquire);
    let insert_index = position.get_absolute(current_index, inner.len());

    let mut new_current_index = current_index;
    if matches!(position, InsertPosition::Replace) {
      inner.clear();
      new_current_index = 0;
    } else if inner.len() > 0 && insert_index <= current_index {
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

    self
      .current_track_index
      .store(new_current_index, Ordering::Release);
    self.track_list_len.store(inner.len(), Ordering::Release);

    Ok(())
  }
}

#[cfg(test)]
mod test {
  use std::{collections::HashSet, path::PathBuf, sync::Arc};

  use super::*;
  use hsm_ipc::{AudioSpec, Track, TrackMetadata};
  use macro_rules_attribute::apply;
  use smol_macros::test;

  struct TrackListRepr {
    track_names: Vec<String>,
    shuffled_track_indicies: Vec<usize>,
    current_track_index: usize,
  }

  impl TrackListRepr {
    async fn new(track_list: &TrackList) -> Self {
      let inner = track_list.inner.lock().await;
      let track_names = inner
        .track_list
        .iter()
        .map(|track| track.metadata().title.clone().unwrap())
        .collect();

      Self {
        track_names,
        shuffled_track_indicies: inner.shuffled_track_indicies.clone(),
        current_track_index: track_list.current_track_index.load(Ordering::Acquire),
      }
    }
  }

  fn dummy_track(title: &str) -> Arc<Track> {
    let track = Track::new(
      PathBuf::from(format!("/{title}")),
      AudioSpec {
        track_id: 0,
        bit_depth: None,
        channel_bitmask: 0,
        channel_count: 0,
        sample_rate: 0,
        total_duration: None,
      },
      TrackMetadata {
        title: Some(title.into()),
        artists: HashSet::new(),
        album: None,
        track_number: None,
        date: None,
        genres: HashSet::new(),
        comments: Vec::new(),
      },
    );

    Arc::new(track)
  }

  async fn dummy_track_list(dummy_track_titles: Vec<&str>) -> TrackList {
    let dummy_tracks: Vec<Arc<Track>> = dummy_track_titles
      .iter()
      .map(|title| dummy_track(title))
      .collect();

    let track_list = TrackList::new();
    track_list
      .insert_tracks(InsertPosition::Start, &dummy_tracks)
      .await
      .unwrap();
    track_list
  }

  #[apply(test!)]
  async fn test_empty_insert() {
    let track_list = TrackList::new();
    let tracks_to_insert = [dummy_track("a"), dummy_track("b"), dummy_track("c")];

    track_list
      .insert_tracks(InsertPosition::Start, &tracks_to_insert)
      .await
      .unwrap();
    let repr = TrackListRepr::new(&track_list).await;

    assert_eq!(repr.track_names, vec!["a", "b", "c"]);
    assert_eq!(repr.shuffled_track_indicies, [0, 1, 2]);
  }

  #[apply(test!)]
  async fn test_start_insert() {
    let track_list = dummy_track_list(vec!["a", "b", "c"]).await;
    let tracks_to_insert = [dummy_track("d"), dummy_track("e"), dummy_track("f")];

    track_list
      .insert_tracks(InsertPosition::Start, &tracks_to_insert)
      .await
      .unwrap();
    let repr = TrackListRepr::new(&track_list).await;

    assert_eq!(repr.track_names, vec!["d", "e", "f", "a", "b", "c"]);
    assert_eq!(repr.shuffled_track_indicies, [0, 1, 2, 3, 4, 5]);
    // Track list should stay on track "a"
    assert_eq!(repr.current_track_index, 3);
  }

  #[apply(test!)]
  async fn test_end_insert() {
    let track_list = dummy_track_list(vec!["a", "b", "c"]).await;
    let tracks_to_insert = [dummy_track("d"), dummy_track("e"), dummy_track("f")];

    track_list
      .insert_tracks(InsertPosition::End, &tracks_to_insert)
      .await
      .unwrap();
    let repr = TrackListRepr::new(&track_list).await;

    assert_eq!(repr.track_names, vec!["a", "b", "c", "d", "e", "f"]);
    assert_eq!(repr.shuffled_track_indicies, [0, 1, 2, 3, 4, 5]);
    // Track list should stay on track "a"
    assert_eq!(repr.current_track_index, 0);
  }
}
