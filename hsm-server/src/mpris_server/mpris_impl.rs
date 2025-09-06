use async_oneshot as oneshot;
use hsm_ipc::{InsertPosition, SeekPosition};
use mpris_server::{
  PlayerInterface, RootInterface,
  zbus::{self, fdo},
};
use smol::channel::{self, Sender};

use crate::audio_server::message::{Message, Query};

use super::conversions::{
  as_dbus_time, as_loop_status, as_playback_status, decode_file_url, from_dbus_time,
  from_loop_status, generate_metadata,
};

pub struct MprisImpl {
  message_tx: Sender<Message>,
  quit_tx: Sender<()>,
}

impl MprisImpl {
  pub fn new(message_tx: channel::Sender<Message>, quit_tx: channel::Sender<()>) -> Self {
    Self {
      message_tx,
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

  async fn try_send(&self, message: Message) -> fdo::Result<()> {
    self
      .message_tx
      .send(message)
      .await
      .map_err(Self::channel_closed_error)
  }

  async fn try_query<T>(&self, query: impl Fn(oneshot::Sender<T>) -> Query) -> fdo::Result<T> {
    let (query_tx, query_rx) = oneshot::oneshot();
    self.try_send(Message::Query(query(query_tx))).await?;
    Ok(query_rx.await.map_err(Self::channel_closed_error)?)
  }
}

impl RootInterface for MprisImpl {
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
    Ok(false)
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

impl PlayerInterface for MprisImpl {
  async fn next(&self) -> fdo::Result<()> {
    self.try_send(Message::NextTrack).await
  }

  async fn previous(&self) -> fdo::Result<()> {
    self.try_send(Message::PreviousTrack { soft: true }).await
  }

  async fn pause(&self) -> fdo::Result<()> {
    self.try_send(Message::Pause).await
  }

  async fn play_pause(&self) -> fdo::Result<()> {
    self.try_send(Message::Toggle).await
  }

  async fn stop(&self) -> fdo::Result<()> {
    self.try_send(Message::Stop).await
  }

  async fn play(&self) -> fdo::Result<()> {
    self.try_send(Message::Play).await
  }

  async fn seek(&self, offset: mpris_server::Time) -> fdo::Result<()> {
    if offset.is_zero() {
      return Ok(());
    }

    let seek_offset = if offset.is_positive() {
      SeekPosition::Forward(from_dbus_time(offset))
    } else {
      SeekPosition::Backward(from_dbus_time(offset))
    };

    self.try_send(Message::Seek(seek_offset)).await
  }

  async fn set_position(
    &self,
    _track_id: mpris_server::TrackId,
    position: mpris_server::Time,
  ) -> fdo::Result<()> {
    if position.is_negative() {
      return Ok(());
    }

    let seek_position = SeekPosition::To(from_dbus_time(position));
    self.try_send(Message::Seek(seek_position)).await
  }

  async fn open_uri(&self, uri: String) -> fdo::Result<()> {
    if let Some(file_path) = decode_file_url(uri) {
      let (tx, rx) = oneshot::oneshot();
      self
        .try_send(Message::InsertTracks {
          paths: vec![file_path],
          position: InsertPosition::End,
          error_tx: tx,
        })
        .await?;

      let errors = rx.await.map_err(Self::channel_closed_error)?;

      match errors.first() {
        Some((_path, error)) => Err(fdo::Error::Failed(error.to_string())),
        None => Ok(()),
      }
    } else {
      Self::unsupported("Unsupported uri type")
    }
  }

  async fn playback_status(&self) -> fdo::Result<mpris_server::PlaybackStatus> {
    let playback_state = self.try_query(Query::PlaybackState).await?;
    Ok(as_playback_status(playback_state))
  }

  async fn loop_status(&self) -> fdo::Result<mpris_server::LoopStatus> {
    let loop_mode = self.try_query(Query::LoopMode).await?;
    Ok(as_loop_status(loop_mode))
  }

  async fn set_loop_status(&self, loop_status: mpris_server::LoopStatus) -> zbus::Result<()> {
    self
      .try_send(Message::SetLoopMode(from_loop_status(loop_status)))
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
    self.try_query(Query::Shuffle).await
  }

  async fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
    self
      .try_send(Message::SetShuffle(shuffle))
      .await
      .map_err(zbus::Error::from)
  }

  async fn metadata(&self) -> fdo::Result<mpris_server::Metadata> {
    let metadata = match self.try_query(Query::CurrentTrack).await? {
      Some(track) => generate_metadata(&track),
      None => mpris_server::Metadata::builder()
        .trackid(mpris_server::TrackId::NO_TRACK)
        .build(),
    };

    Ok(metadata)
  }

  async fn volume(&self) -> fdo::Result<mpris_server::Volume> {
    self
      .try_query(Query::Volume)
      .await
      .map(|volume| volume.into())
  }

  async fn set_volume(&self, volume: mpris_server::Volume) -> zbus::Result<()> {
    self
      .try_send(Message::SetVolume(volume as f32))
      .await
      .map_err(zbus::Error::from)
  }

  async fn position(&self) -> fdo::Result<mpris_server::Time> {
    self.try_query(Query::Position).await.map(as_dbus_time)
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
