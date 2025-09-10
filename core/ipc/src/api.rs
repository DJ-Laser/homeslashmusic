use std::fmt::Debug;

use serde::{Serialize, de::DeserializeOwned};

pub mod client;
pub mod requests;
pub mod server;
mod types;

pub use types::*;

pub(crate) mod private {

  use std::fmt::Debug;

  use serde::{Serialize, de::DeserializeOwned};

  use super::requests;
  pub trait SealedRequest: Debug + Clone + Serialize + DeserializeOwned {
    fn qualified_request(self) -> requests::private::QualifiedRequest;
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
