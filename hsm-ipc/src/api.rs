use std::fmt::Debug;

use serde::{Serialize, de::DeserializeOwned};

pub mod requests;
pub mod responses;

mod sealed {
  use super::*;
  pub trait SealedRequest: Debug + Clone + Serialize + DeserializeOwned {}
}

/// Request sent to the hsm server
pub trait Request: sealed::SealedRequest {
  type Response: Debug + Clone + Serialize + DeserializeOwned;
}

/// Reply from the hsm server
///
/// Every request gets one reply.
///
/// * If an error had occurred, it will be an `Reply::Err`.
/// * If the request does not need any particular response, it will be
///   `Reply::Ok(Response::Handled)`. Kind of like an `Ok(())`.
pub type Reply<R>
where
  R: Request,
= Result<<R as Request>::Response, String>;

//pub fn get_request_data();
