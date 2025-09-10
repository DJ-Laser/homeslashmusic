use std::{iter::FusedIterator, ops::Index};

use hsm_ipc::{Track, TrackListSnapshot, TrackListUpdate};
use serde::{Deserialize, Serialize};

/// A representation of the player's track list
/// `track_list.len()` will always be equal to `shuffle_indicies.len()`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackList {
  track_list: Vec<Track>,
  shuffle_indicies: Vec<usize>,
  needs_sync: bool,
}

impl TrackList {
  pub fn new() -> Self {
    Self {
      track_list: Vec::new(),
      shuffle_indicies: Vec::new(),
      needs_sync: false,
    }
  }

  pub fn from_snapshot(snapshot: TrackListSnapshot) -> Self {
    debug_assert_eq!(snapshot.track_list.len(), snapshot.shuffle_indicies.len());

    Self {
      track_list: snapshot.track_list,
      shuffle_indicies: snapshot.shuffle_indicies,
      needs_sync: false,
    }
  }

  pub fn len(&self) -> usize {
    debug_assert_eq!(self.track_list.len(), self.shuffle_indicies.len());
    self.track_list.len()
  }

  pub fn needs_sync(&self) -> bool {
    !self.needs_sync
  }

  /// Replaces this `TrackList` in place with the contents of `snapshot`
  ///
  /// This will also reset `needs_sync`, allowing further updates to be applied normally again
  pub fn sync(&mut self, snapshot: TrackListSnapshot) {
    debug_assert_eq!(snapshot.track_list.len(), snapshot.shuffle_indicies.len());

    self.track_list = snapshot.track_list;
    self.shuffle_indicies = snapshot.shuffle_indicies;
    self.needs_sync = false;
  }

  /// Attempts to update the `TrackList` state based on `update`
  ///
  /// Returns `Err` if the update would cause the `track_list` `shuffle_indicies`
  /// to have different lengths. This usually indicates that the client is out of sync with the server.
  ///
  /// If this occurs `needs_sync` will be set to true, however incorrect updates
  /// may still be applied if they match the lengths of `track_list` and `shuffle_indicies`
  ///
  /// `TrackListUpdate::Replace` and `TrackListUpdate::Clear` *will* reset `needs_sync`
  /// because they specify the entire known state of the `TrackList`
  pub fn update(&mut self, update: TrackListUpdate) -> Result<(), ()> {
    debug_assert_eq!(self.track_list.len(), self.shuffle_indicies.len());

    match update {
      TrackListUpdate::Insert {
        index,
        tracks,
        new_shuffle_indicies,
      } => {
        let new_len = self.track_list.len() + tracks.len();
        if new_len != new_shuffle_indicies.len() {
          self.needs_sync = true;
          return Err(());
        }

        self.track_list.splice(index..index, tracks);
        self.shuffle_indicies = new_shuffle_indicies;
      }

      TrackListUpdate::Remove {
        removed_indicies,
        new_shuffle_indicies,
      } => {
        let new_len = self.track_list.len() - removed_indicies.len();
        if new_len != new_shuffle_indicies.len() {
          self.needs_sync = true;
          return Err(());
        }

        let mut index = 0;
        self
          .track_list
          .retain(|_| (removed_indicies.contains(&index), index += 1).0);
        self.shuffle_indicies = new_shuffle_indicies;
      }

      TrackListUpdate::Replace(track_list) => {
        self.sync(track_list);
      }

      TrackListUpdate::Clear => {
        self.track_list.clear();
        self.shuffle_indicies.clear();
        self.needs_sync = false;
      }

      TrackListUpdate::Shuffle {
        new_shuffle_indicies,
      } => {
        if self.track_list.len() != new_shuffle_indicies.len() {
          self.needs_sync = true;
          return Err(());
        }

        self.shuffle_indicies = new_shuffle_indicies;
      }
    }

    debug_assert_eq!(self.track_list.len(), self.shuffle_indicies.len());

    Ok(())
  }

  pub fn iter(&self) -> TrackListIter {
    TrackListIter::new(self)
  }
}

impl Index<usize> for TrackList {
  type Output = Track;

  fn index(&self, index: usize) -> &Self::Output {
    debug_assert_eq!(self.track_list.len(), self.shuffle_indicies.len());
    &self.track_list[self.shuffle_indicies[index]]
  }
}

pub struct TrackListIter<'a> {
  track_list: &'a TrackList,
  index: usize,
}

impl<'a> TrackListIter<'a> {
  fn new(track_list: &'a TrackList) -> Self {
    Self {
      track_list,
      index: 0,
    }
  }
}

impl<'a> Iterator for TrackListIter<'a> {
  type Item = &'a Track;

  fn next(&mut self) -> Option<Self::Item> {
    if self.index >= self.track_list.len() {
      return None;
    }

    Some((&self.track_list[self.index], self.index += 1).0)
  }
}

impl<'a> FusedIterator for TrackListIter<'a> {}
