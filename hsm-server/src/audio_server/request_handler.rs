use std::{path::PathBuf, time::Duration};

use hsm_ipc::{
  LoopMode, PlaybackState, Track, TrackListSnapshot, requests, server::RequestHandler,
};

use super::{AudioServer, AudioServerError};

impl RequestHandler for AudioServer {
  type Error = AudioServerError;

  async fn handle_query_version(
    &self,
    _request: requests::QueryVersion,
  ) -> Result<hsm_ipc::Version, Self::Error> {
    Ok(hsm_ipc::version())
  }

  async fn handle_query_playback_state(
    &self,
    _request: requests::QueryPlaybackState,
  ) -> Result<PlaybackState, Self::Error> {
    Ok(self.player.playback_state())
  }

  async fn handle_play(&self, _request: requests::Play) -> Result<(), Self::Error> {
    Ok(self.player.play().await?)
  }

  async fn handle_pause(&self, _request: requests::Pause) -> Result<(), Self::Error> {
    Ok(self.player.pause().await?)
  }

  async fn handle_stop_playback(
    &self,
    _request: requests::StopPlayback,
  ) -> Result<(), Self::Error> {
    Ok(self.player.stop().await?)
  }

  async fn handle_toggle_playback(
    &self,
    _request: requests::TogglePlayback,
  ) -> Result<(), Self::Error> {
    Ok(self.player.toggle_playback().await?)
  }

  async fn handle_query_current_track(
    &self,
    _request: requests::QueryCurrentTrack,
  ) -> Result<Option<Track>, Self::Error> {
    let track = self.player.current_track().await;

    Ok(track)
  }

  async fn handle_query_current_track_index(
    &self,
    _request: requests::QueryCurrentTrackIndex,
  ) -> Result<usize, Self::Error> {
    Ok(self.player.current_track_index())
  }

  async fn handle_next_track(&self, _request: requests::NextTrack) -> Result<(), Self::Error> {
    Ok(self.player.go_to_next_track().await?)
  }

  async fn handle_previous_track(
    &self,
    requests::PreviousTrack { soft }: requests::PreviousTrack,
  ) -> Result<(), Self::Error> {
    Ok(self.player.go_to_previous_track(soft).await?)
  }

  async fn handle_query_loop_mode(
    &self,
    _request: requests::QueryLoopMode,
  ) -> Result<LoopMode, Self::Error> {
    Ok(self.player.loop_mode())
  }

  async fn handle_set_loop_mode(
    &self,
    requests::SetLoopMode(loop_mode): requests::SetLoopMode,
  ) -> Result<(), Self::Error> {
    Ok(self.player.set_loop_mode(loop_mode).await?)
  }

  async fn handle_query_shuffle(
    &self,
    _request: requests::QueryShuffle,
  ) -> Result<bool, Self::Error> {
    Ok(self.player.shuffle().await)
  }

  async fn handle_set_shuffle(
    &self,
    requests::SetShuffle(shuffle): requests::SetShuffle,
  ) -> Result<(), Self::Error> {
    Ok(self.player.set_shuffle(shuffle).await?)
  }

  async fn handle_query_volume(&self, _request: requests::QueryVolume) -> Result<f32, Self::Error> {
    Ok(self.player.volume().await)
  }

  async fn handle_set_volume(
    &self,
    requests::SetVolume(volume): requests::SetVolume,
  ) -> Result<(), Self::Error> {
    Ok(self.player.set_volume(volume).await?)
  }

  async fn handle_query_position(
    &self,
    _request: requests::QueryPosition,
  ) -> Result<Duration, Self::Error> {
    Ok(self.player.position().await)
  }

  async fn handle_seek(
    &self,
    requests::Seek(seek_position): requests::Seek,
  ) -> Result<(), Self::Error> {
    Ok(self.player.seek(seek_position).await?)
  }

  async fn handle_query_track_list(
    &self,
    _request: requests::QueryTrackList,
  ) -> Result<TrackListSnapshot, Self::Error> {
    Ok(self.player.get_track_list().await)
  }

  async fn handle_clear_tracks(&self, _request: requests::ClearTracks) -> Result<(), Self::Error> {
    Ok(self.player.clear_tracks().await?)
  }

  async fn handle_load_tracks(
    &self,
    requests::LoadTracks(position, paths): requests::LoadTracks,
  ) -> Result<Vec<(PathBuf, String)>, Self::Error> {
    println!("Loading tracks: {:?}", paths);
    let (tracks, errors) = self.track_cache.get_or_load_tracks(paths).await;

    for (path, error) in errors.iter() {
      eprintln!("Could not load track {path:?}: {error}")
    }

    for track in tracks.iter() {
      println!("Loaded track {:?}", track.file_path());
    }

    (self.player.insert_tracks(position, &tracks).await)?;

    Ok(
      errors
        .into_iter()
        .map(|(path, error)| (path, error.to_string()))
        .collect(),
    )
  }
}
