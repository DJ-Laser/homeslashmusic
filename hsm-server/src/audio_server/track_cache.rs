use std::{
  path::PathBuf,
  pin::pin,
  sync::{Arc, Weak},
};

use dashmap::DashMap;
use hsm_ipc::Track;
use smol::stream::StreamExt;

use super::track::{self, LoadTrackError};

pub struct TrackCache {
  loaded_tracks: DashMap<PathBuf, Weak<Track>>,
}

impl TrackCache {
  pub fn new() -> Self {
    Self {
      loaded_tracks: DashMap::new(),
    }
  }

  async fn get_or_load_track(&self, path: PathBuf) -> Result<Arc<Track>, LoadTrackError> {
    let Some(track) = self
      .loaded_tracks
      .get(&path)
      .and_then(|weak| weak.upgrade())
    else {
      let track = Arc::new(track::load_file(path).await?);
      self
        .loaded_tracks
        .insert(track.file_path().clone(), Arc::downgrade(&track));

      return Ok(track);
    };

    return Ok(track);
  }

  pub async fn get_or_load_tracks(
    &self,
    paths: Vec<PathBuf>,
  ) -> (Vec<Arc<Track>>, Vec<(PathBuf, LoadTrackError)>) {
    let mut tracks = Vec::new();
    let mut errors = Vec::new();

    for path in paths {
      let mut cannonical_paths = pin!(track::search_file_or_directory(path).await);
      while let Some(res) = cannonical_paths.next().await {
        match res {
          Ok(track_path) => match self.get_or_load_track(track_path.clone()).await {
            Ok(track) => tracks.push(track),
            Err(error) => errors.push((track_path, error)),
          },
          Err(error) => errors.push(error),
        }
      }
    }

    (tracks, errors)
  }
}
