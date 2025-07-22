use mpris_server::{
  LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, RootInterface, Time,
  TrackId, Volume,
  zbus::{self, fdo},
};
use smol::channel::Sender;

use crate::audio_server::message::{Message, PlaybackControl};

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

  async fn try_send(&self, message: Message) -> fdo::Result<()> {
    self
      .message_tx
      .send(message)
      .await
      .map_err(|_| fdo::Error::Failed("Channel was unexpectedly closed".into()))
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
    self
      .try_send(Message::Playback(PlaybackControl::Pause))
      .await
  }

  async fn play_pause(&self) -> fdo::Result<()> {
    self
      .try_send(Message::Playback(PlaybackControl::Toggle))
      .await
  }

  async fn stop(&self) -> fdo::Result<()> {
    Self::unsupported("Stop is not supported")
  }

  async fn play(&self) -> fdo::Result<()> {
    self
      .try_send(Message::Playback(PlaybackControl::Play))
      .await
  }

  async fn seek(&self, _offset: Time) -> fdo::Result<()> {
    Self::unsupported("Seek is not supported")
  }

  async fn set_position(&self, _track_id: TrackId, _position: Time) -> fdo::Result<()> {
    Self::unsupported("SetPosition is not supported")
  }

  async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
    Self::unsupported("OpenUri is not supported")
  }

  async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
    Self::unsupported("PlaybackStatus is not supported")
  }

  async fn loop_status(&self) -> fdo::Result<LoopStatus> {
    Ok(LoopStatus::None)
  }

  async fn set_loop_status(&self, _loop_status: LoopStatus) -> zbus::Result<()> {
    Self::unsupported_set("SetLoopStatus is not supported")
  }

  async fn rate(&self) -> fdo::Result<PlaybackRate> {
    Ok(1.0)
  }

  async fn set_rate(&self, _rate: PlaybackRate) -> zbus::Result<()> {
    Self::unsupported_set("SetRate is not supported")
  }

  async fn shuffle(&self) -> fdo::Result<bool> {
    Ok(false)
  }

  async fn set_shuffle(&self, _shuffle: bool) -> zbus::Result<()> {
    Self::unsupported_set("SetShuffle is not supported")
  }

  async fn metadata(&self) -> fdo::Result<Metadata> {
    Self::unsupported("Metadata is not supported")
  }

  async fn volume(&self) -> fdo::Result<Volume> {
    Self::unsupported("Volume is not supported")
  }

  async fn set_volume(&self, _volume: Volume) -> zbus::Result<()> {
    Self::unsupported_set("SetVolume is not supported")
  }

  async fn position(&self) -> fdo::Result<Time> {
    Self::unsupported("Position is not supported")
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
    Ok(false)
  }

  async fn can_control(&self) -> fdo::Result<bool> {
    Ok(true)
  }
}
