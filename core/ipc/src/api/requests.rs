use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
  InsertPosition, LoopMode, SeekPosition,
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
pub struct LoadTracks {
  pub paths: Vec<PathBuf>,
  pub position: InsertPosition,
}
impl LoadTracks {
  pub fn new(paths: Vec<PathBuf>, position: InsertPosition) -> Self {
    Self { paths, position }
  }
}
impl SealedRequest for LoadTracks {
  fn qualified_request(self) -> QualifiedRequest {
    QualifiedRequest::LoadTrack(self)
  }
}
impl Request for LoadTracks {
  type Response = Vec<(PathBuf, String)>;
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

#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct ClearTracks;
impl SealedRequest for ClearTracks {
  fn qualified_request(self) -> QualifiedRequest {
    QualifiedRequest::ClearTracks(self)
  }
}
impl Request for ClearTracks {
  type Response = ();
}
