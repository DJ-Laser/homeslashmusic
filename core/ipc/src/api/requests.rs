use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
  LoopMode, SeekPosition,
  client::{Request, private::SealedRequest},
  responses,
  server::private::QualifiedRequest,
};

/// The `hsm_ipc::version()` of the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version;
impl SealedRequest for Version {
  fn qualified_request(self) -> QualifiedRequest {
    QualifiedRequest::Version(self)
  }
}
impl Request for Version {
  type Response = responses::Version;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Playback {
  Play,
  Pause,
  Toggle,
  Stop,
}
impl SealedRequest for Playback {
  fn qualified_request(self) -> QualifiedRequest {
    QualifiedRequest::Playback(self)
  }
}
impl Request for Playback {
  type Response = ();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Set {
  Volume(f32),
  LoopMode(LoopMode),
}
impl SealedRequest for Set {
  fn qualified_request(self) -> QualifiedRequest {
    QualifiedRequest::Set(self)
  }
}
impl Request for Set {
  type Response = ();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTrack {
  pub path: PathBuf,
}
impl LoadTrack {
  pub fn new(path: PathBuf) -> Self {
    Self { path }
  }
}
impl SealedRequest for LoadTrack {
  fn qualified_request(self) -> QualifiedRequest {
    QualifiedRequest::LoadTrack(self)
  }
}
impl Request for LoadTrack {
  type Response = ();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seek {
  pub seek_position: SeekPosition,
}
impl Seek {
  pub fn new(seek_position: SeekPosition) -> Self {
    Self { seek_position }
  }
}
impl SealedRequest for Seek {
  fn qualified_request(self) -> QualifiedRequest {
    QualifiedRequest::Seek(self)
  }
}
impl Request for Seek {
  type Response = ();
}
