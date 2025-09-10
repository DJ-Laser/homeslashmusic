use std::{fmt::Debug, time::Duration};

use serde::{Serialize, de::DeserializeOwned};

pub mod client;
pub mod requests;
pub mod server;
mod types;

pub use requests::private::QualifiedRequest;
pub use types::*;

pub(crate) mod private {
  use std::fmt::Debug;

  use serde::{Serialize, de::DeserializeOwned};

  use super::QualifiedRequest;

  pub trait SealedRequest:
    Debug + Clone + Serialize + DeserializeOwned + Into<QualifiedRequest>
  {
  }
}

/// Request sent to the hsm server
pub trait Request: private::SealedRequest {
  type Response: Debug + Clone + Serialize + DeserializeOwned;
}

/// Reply from the hsm server
///
/// Every request gets one reply.
///
/// * If an error had occurred, it will be an `Reply::Err`.
/// * If the request does not need any particular response, it will be
///   `Reply::Ok(Response::Handled)`. Kind of like an `Ok(())`.
#[allow(type_alias_bounds)]
pub type Reply<R>
where
  R: Request,
= Result<<R as Request>::Response, String>;

/// An event than can be sent from the serverasynchronously at any time.
#[derive(Debug, Clone)]
pub enum Event {
  PlaybackStateChanged(PlaybackState),
  LoopModeChanged(LoopMode),
  ShuffleChanged(bool),
  VolumeChanged(f32),
  Seeked(Duration),
}
