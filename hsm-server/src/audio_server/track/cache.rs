use std::{
  path::PathBuf,
  sync::{Arc, Weak},
};

use dashmap::DashMap;
use smol::{fs, stream::StreamExt};

use super::{LoadTrackError, LoadedTrack};

type Tracks = Vec<Arc<LoadedTrack>>;
type Errors = Vec<(PathBuf, LoadTrackError)>;

pub struct TrackCache {
  loaded_tracks: DashMap<PathBuf, Weak<LoadedTrack>>,
}

impl TrackCache {
  pub fn new() -> Self {
    Self {
      loaded_tracks: DashMap::new(),
    }
  }

  /// Does not search directories or cannonicalize paths, only provide cannonical paths to files
  async fn get_or_load_track(
    &self,
    path: PathBuf,
  ) -> Result<Arc<LoadedTrack>, (PathBuf, LoadTrackError)> {
    let cannnonical_path = super::get_cannonical_track_path(&path)
      .await
      .map_err(|error| (path.clone(), error))?;

    let Some(track) = self
      .loaded_tracks
      .get(&cannnonical_path)
      .and_then(|weak| weak.upgrade())
    else {
      let track = Arc::new(
        super::load_file(cannnonical_path)
          .await
          .map_err(|error| (path, error))?,
      );
      self
        .loaded_tracks
        .insert(track.file_path().to_path_buf(), Arc::downgrade(&track));

      return Ok(track);
    };

    return Ok(track);
  }

  /// Sorts by title, then track number, then album
  /// Tracks without these will be sorted to the end
  async fn sort_tracks(&self, tracks: &mut Tracks) {
    // Sort by title if available, othewise by file name
    fn get_track_title(track: &Arc<LoadedTrack>) -> String {
      track
        .metadata()
        .title
        .as_ref()
        .map(|s| s.to_lowercase())
        .or_else(|| {
          track
            .file_path()
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or("".into())
    }

    tracks.sort_by_key(|track| get_track_title(track));
    tracks.sort_by_key(|track| track.metadata().track_number);
    tracks.sort_by(|track_a, track_b| track_a.metadata().album.cmp(&track_b.metadata().album));
  }

  async fn search_directory(&self, path: PathBuf, outer_tracks: &mut Tracks, errors: &mut Errors) {
    let mut tracks = Vec::new();

    let mut entries = match fs::read_dir(&path).await {
      Ok(files) => files,
      Err(error) => {
        return errors.push((path, LoadTrackError::ReadDirFailed(error)));
      }
    };

    while let Some(entry) = entries.next().await {
      let entry_path = match entry {
        Ok(entry) => entry.path(),
        Err(error) => {
          errors.push((path.clone(), LoadTrackError::ReadDirFailed(error)));
          continue;
        }
      };

      Box::pin(self.search_file_or_directory(entry_path, &mut tracks, errors)).await;
    }

    self.sort_tracks(&mut tracks).await;
    outer_tracks.extend(tracks);
  }

  async fn search_file_or_directory(
    &self,
    path: PathBuf,
    tracks: &mut Tracks,
    errors: &mut Errors,
  ) {
    let metadata = match fs::metadata(&path).await {
      Ok(metadata) => metadata,
      Err(error) => {
        return errors.push((path, LoadTrackError::OpenFailed(error)));
      }
    };

    if metadata.is_dir() {
      self.search_directory(path, tracks, errors).await;
    } else {
      match self.get_or_load_track(path).await {
        Ok(track) => tracks.push(track),
        Err(error) => errors.push(error),
      }
    }
  }

  pub async fn get_or_load_tracks(&self, paths: Vec<PathBuf>) -> (Tracks, Errors) {
    let mut tracks = Vec::new();
    let mut errors = Vec::new();

    for path in paths {
      self
        .search_file_or_directory(path, &mut tracks, &mut errors)
        .await
    }

    (tracks, errors)
  }
}
