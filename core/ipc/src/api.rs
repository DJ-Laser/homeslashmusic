pub mod client;
pub mod requests;
pub mod responses;
pub mod server;
mod types;

pub use types::*;

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
  R: client::Request,
= Result<<R as client::Request>::Response, String>;
