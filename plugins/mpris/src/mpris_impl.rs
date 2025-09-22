use hsm_ipc::{InsertPosition, Request, SeekOffset, TrackId, requests};
use hsm_plugin::RequestSender;
use mpris_server::{
  PlayerInterface, RootInterface, TrackListInterface,
  zbus::{self, fdo},
};
use smol::channel::{self, Sender};

use crate::conversions::{
  from_dbus_track_id, from_real_dbus_track_id, generate_empty_metadata, into_dbus_track_id,
};

use super::conversions::{
  as_dbus_time, as_loop_status, as_playback_status, decode_file_url, from_dbus_time,
  from_loop_status, generate_metadata,
};

pub struct MprisImpl<Tx> {
  request_tx: Tx,
  quit_tx: Sender<()>,
}

impl<Tx> MprisImpl<Tx> {
  pub fn new(request_tx: Tx, quit_tx: channel::Sender<()>) -> Self {
    Self {
      request_tx,
      quit_tx,
    }
  }

  fn unsupported<T>(message: &str) -> fdo::Result<T> {
    Err(fdo::Error::NotSupported(message.into()))
  }

  fn unsupported_set<T>(message: &str) -> zbus::Result<T> {
    Self::unsupported(message).map_err(zbus::Error::from)
  }

  fn channel_closed_error<T>(_t: T) -> fdo::Error {
    fdo::Error::Failed("Channel was unexpectedly closed".into())
  }
}

impl<Tx: RequestSender + Send + Sync> MprisImpl<Tx> {
  async fn try_send<R: Request>(&self, request: R) -> fdo::Result<R::Response>
  where
    R::Response: Send,
  {
    self
      .request_tx
      .send_request(request)
      .await
      .map_err(Self::channel_closed_error)
  }

  async fn load_uri(
    &self,
    uri: String,
    position: InsertPosition,
    set_as_current: bool,
  ) -> fdo::Result<()> {
    let Some(file_path) = decode_file_url(uri) else {
      return Self::unsupported("Unsupported uri type");
    };

    let errors = self
      .try_send(requests::LoadTracks {
        position,
        paths: vec![file_path],
        set_as_current,
      })
      .await?;

    match errors.first() {
      Some((_path, error)) => Err(fdo::Error::Failed(error.to_string())),
      None => Ok(()),
    }
  }
}

impl<Tx: RequestSender + Send + Sync> RootInterface for MprisImpl<Tx> {
  async fn raise(&self) -> fdo::Result<()> {
    Self::unsupported("Raise is not supported")
  }

  async fn quit(&self) -> fdo::Result<()> {
    match self.quit_tx.try_send(()) {
      Ok(()) => Ok(()),
      Err(channel::TrySendError::Closed(e)) => Err(Self::channel_closed_error(e)),

      // If the channel is full, a quit message was already sent
      Err(channel::TrySendError::Full(_)) => Ok(()),
    }
  }

  async fn can_quit(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  async fn fullscreen(&self) -> fdo::Result<bool> {
    Ok(false)
  }

  async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
    Self::unsupported_set("Fullscreen is not supported")
  }

  async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
    Ok(false)
  }

  async fn can_raise(&self) -> fdo::Result<bool> {
    Ok(false)
  }

  async fn has_track_list(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  async fn identity(&self) -> fdo::Result<String> {
    Ok("~/Music".into())
  }

  async fn desktop_entry(&self) -> fdo::Result<String> {
    Ok("homeslashmusic".into())
  }

  async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
    Ok(vec!["file".into()])
  }

  async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
    Ok(vec![
      "audio/aac".into(),
      "audio/mpeg".into(),
      "audio/ogg".into(),
      "audio/wav".into(),
    ])
  }
}

impl<Tx: RequestSender + Send + Sync> PlayerInterface for MprisImpl<Tx> {
  async fn next(&self) -> fdo::Result<()> {
    self.try_send(requests::GoToNextTrack).await
  }

  async fn previous(&self) -> fdo::Result<()> {
    self
      .try_send(requests::GoToPreviousTrack { soft: true })
      .await
  }

  async fn pause(&self) -> fdo::Result<()> {
    self.try_send(requests::Pause).await
  }

  async fn play_pause(&self) -> fdo::Result<()> {
    self.try_send(requests::TogglePlayback).await
  }

  async fn stop(&self) -> fdo::Result<()> {
    self.try_send(requests::StopPlayback).await
  }

  async fn play(&self) -> fdo::Result<()> {
    self.try_send(requests::Play).await
  }

  async fn seek(&self, offset: mpris_server::Time) -> fdo::Result<()> {
    if offset.is_zero() {
      return Ok(());
    }

    let seek_offset = if offset.is_positive() {
      SeekOffset::Forward(from_dbus_time(offset))
    } else {
      SeekOffset::Backward(from_dbus_time(offset))
    };

    self.try_send(requests::SeekCurrent(seek_offset)).await
  }

  async fn set_position(
    &self,
    track_id: mpris_server::TrackId,
    position: mpris_server::Time,
  ) -> fdo::Result<()> {
    if position.is_negative() {
      return Ok(());
    }

    let track_id = from_real_dbus_track_id(track_id)?;

    self
      .try_send(requests::SeekTo(track_id, from_dbus_time(position)))
      .await?;

    Ok(())
  }

  async fn open_uri(&self, uri: String) -> fdo::Result<()> {
    self.load_uri(uri, InsertPosition::End, false).await
  }

  async fn playback_status(&self) -> fdo::Result<mpris_server::PlaybackStatus> {
    let playback_state = self.try_send(requests::QueryPlaybackState).await?;
    Ok(as_playback_status(playback_state))
  }

  async fn loop_status(&self) -> fdo::Result<mpris_server::LoopStatus> {
    let loop_mode = self.try_send(requests::QueryLoopMode).await?;
    Ok(as_loop_status(loop_mode))
  }

  async fn set_loop_status(&self, loop_status: mpris_server::LoopStatus) -> zbus::Result<()> {
    self
      .try_send(requests::SetLoopMode(from_loop_status(loop_status)))
      .await
      .map_err(zbus::Error::from)
  }

  async fn rate(&self) -> fdo::Result<mpris_server::PlaybackRate> {
    Ok(1.0)
  }

  async fn set_rate(&self, rate: mpris_server::PlaybackRate) -> zbus::Result<()> {
    if rate == 0.0 {
      self.pause().await?;
    } else if rate != 1.0 {
      Self::unsupported_set("Unsupported rate")?
    }

    Ok(())
  }

  async fn shuffle(&self) -> fdo::Result<bool> {
    self.try_send(requests::QueryShuffle).await
  }

  async fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
    self
      .try_send(requests::SetShuffle(shuffle))
      .await
      .map_err(zbus::Error::from)
  }

  async fn metadata(&self) -> fdo::Result<mpris_server::Metadata> {
    let metadata = match self.try_send(requests::QueryCurrentTrack).await? {
      Some(track) => generate_metadata(track),
      None => mpris_server::Metadata::builder()
        .trackid(mpris_server::TrackId::NO_TRACK)
        .build(),
    };

    Ok(metadata)
  }

  async fn volume(&self) -> fdo::Result<mpris_server::Volume> {
    self
      .try_send(requests::QueryVolume)
      .await
      .map(|volume| volume.into())
  }

  async fn set_volume(&self, volume: mpris_server::Volume) -> zbus::Result<()> {
    self
      .try_send(requests::SetVolume(volume as f32))
      .await
      .map_err(zbus::Error::from)
  }

  async fn position(&self) -> fdo::Result<mpris_server::Time> {
    self
      .try_send(requests::QueryPosition)
      .await
      .map(as_dbus_time)
  }

  async fn minimum_rate(&self) -> fdo::Result<mpris_server::PlaybackRate> {
    Ok(1.0)
  }

  async fn maximum_rate(&self) -> fdo::Result<mpris_server::PlaybackRate> {
    Ok(1.0)
  }

  async fn can_go_next(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  async fn can_go_previous(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  async fn can_play(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  async fn can_pause(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  async fn can_seek(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  async fn can_control(&self) -> fdo::Result<bool> {
    Ok(true)
  }
}

impl<Tx: RequestSender + Send + Sync> TrackListInterface for MprisImpl<Tx> {
  async fn get_tracks_metadata(
    &self,
    track_ids: Vec<mpris_server::TrackId>,
  ) -> fdo::Result<Vec<mpris_server::Metadata>> {
    let track_ids = track_ids
      .into_iter()
      .map(|track_id| from_real_dbus_track_id(track_id))
      .collect::<Result<Vec<TrackId>, _>>()?;

    let mut tracks = self.try_send(requests::QueryQueueTracks).await?;

    let metadata_list = track_ids
      .into_iter()
      .map(
        |track_id| match tracks.iter().position(|track| track.id == track_id) {
          Some(index) => {
            let track = tracks.swap_remove(index);
            generate_metadata(track)
          }
          None => generate_empty_metadata(track_id),
        },
      )
      .collect();

    Ok(metadata_list)
  }

  async fn add_track(
    &self,
    uri: mpris_server::Uri,
    after_track: mpris_server::TrackId,
    set_as_current: bool,
  ) -> fdo::Result<()> {
    let after_track = from_dbus_track_id(after_track)?;
    let insert_position = match after_track {
      Some(track_id) => InsertPosition::After(track_id),
      None => InsertPosition::Start,
    };

    self.load_uri(uri, insert_position, set_as_current).await?;

    Ok(())
  }

  async fn remove_track(&self, track_id: mpris_server::TrackId) -> fdo::Result<()> {
    todo!()
  }

  async fn go_to(&self, track_id: mpris_server::TrackId) -> fdo::Result<()> {
    let track_id = from_real_dbus_track_id(track_id)?;
    self.try_send(requests::GoToTrack(track_id)).await?;

    Ok(())
  }

  async fn tracks(&self) -> fdo::Result<Vec<mpris_server::TrackId>> {
    let track_ids = self.try_send(requests::QueryQueueTrackIds).await?;

    let track_ids = track_ids
      .into_iter()
      .map(|track_id| into_dbus_track_id(track_id))
      .collect();

    Ok(track_ids)
  }

  async fn can_edit_tracks(&self) -> fdo::Result<bool> {
    Ok(true)
  }
}
