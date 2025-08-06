use std::{path::PathBuf, time::Duration};

use async_oneshot as oneshot;
use hsm_ipc::{InsertPosition, LoopMode, SeekPosition};
use mpris_server::{
  LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, RootInterface, Time,
  TrackId, Volume,
  zbus::{self, fdo},
};
use smol::channel::Sender;

use crate::audio_server::message::{Message, Query};

use super::{loop_status, metadata, playback_status};

pub struct MprisImpl {
  message_tx: Sender<Message>,
}

impl MprisImpl {
  pub fn new(message_tx: Sender<Message>) -> Self {
    Self { message_tx }
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
    todo!() // Will technically quit lol
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
    Self::unsupported("Next is not supported")
  }

  async fn previous(&self) -> fdo::Result<()> {
    Self::unsupported("Previous is not supported")
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

  async fn seek(&self, offset: Time) -> fdo::Result<()> {
    if offset.is_zero() {
      return Ok(());
    }

    let seek_offset = if offset.is_positive() {
      SeekPosition::Forward(Duration::from_micros(offset.as_micros() as u64))
    } else {
      SeekPosition::Backward(Duration::from_micros(offset.abs().as_micros() as u64))
    };

    self.try_send(Message::Seek(seek_offset)).await
  }

  async fn set_position(&self, _track_id: TrackId, position: Time) -> fdo::Result<()> {
    if position.is_negative() {
      return Ok(());
    }

    let seek_position = SeekPosition::To(Duration::from_micros(position.as_micros() as u64));
    self.try_send(Message::Seek(seek_position)).await
  }

  async fn open_uri(&self, uri: String) -> fdo::Result<()> {
    if let Some(file_path) = uri.strip_prefix("file://") {
      let file_path = PathBuf::from(file_path);

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

  async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
    let playback_state = self.try_query(Query::PlaybackState).await?;
    Ok(playback_status(playback_state))
  }

  async fn loop_status(&self) -> fdo::Result<LoopStatus> {
    let loop_mode = self.try_query(Query::LoopMode).await?;
    Ok(loop_status(loop_mode))
  }

  async fn set_loop_status(&self, loop_status: LoopStatus) -> zbus::Result<()> {
    let loop_mode = match loop_status {
      LoopStatus::None => LoopMode::None,
      LoopStatus::Track => LoopMode::Track,
      LoopStatus::Playlist => LoopMode::Playlist,
    };

    self
      .try_send(Message::SetLoopMode(loop_mode))
      .await
      .map_err(zbus::Error::from)
  }

  async fn rate(&self) -> fdo::Result<PlaybackRate> {
    Ok(1.0)
  }

  async fn set_rate(&self, rate: PlaybackRate) -> zbus::Result<()> {
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

  async fn metadata(&self) -> fdo::Result<Metadata> {
    if let Some(track) = self.try_query(Query::CurrentTrack).await? {
      return Ok(metadata(&track));
    }

    return Ok(Metadata::new());
  }

  async fn volume(&self) -> fdo::Result<Volume> {
    self
      .try_query(Query::Volume)
      .await
      .map(|volume| volume.into())
  }

  async fn set_volume(&self, volume: Volume) -> zbus::Result<()> {
    self
      .try_send(Message::SetVolume(volume as f32))
      .await
      .map_err(zbus::Error::from)
  }

  async fn position(&self) -> fdo::Result<Time> {
    self
      .try_query(Query::Position)
      .await
      .map(|position| Time::from_micros(position.as_micros() as i64))
  }

  async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
    Ok(1.0)
  }

  async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
    Ok(1.0)
  }

  async fn can_go_next(&self) -> fdo::Result<bool> {
    Ok(false)
  }

  async fn can_go_previous(&self) -> fdo::Result<bool> {
    Ok(false)
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
