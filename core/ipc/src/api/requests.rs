use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
  client::Request, client::private::SealedRequest, responses, server::private::QualifiedRequest,
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
}
impl SealedRequest for Playback {
  fn qualified_request(self) -> QualifiedRequest {
    QualifiedRequest::Playback(self)
  }
}
impl Request for Playback {
  type Response = responses::Handled;
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
  type Response = responses::Handled;
}
