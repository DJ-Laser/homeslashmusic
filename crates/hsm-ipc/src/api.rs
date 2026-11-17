pub mod client;
pub mod events;
pub mod requests;
pub mod server;
mod types;

use std::fmt::Debug;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub use events::*;
use thiserror::Error;
pub use types::*;

use crate::requests::private::QualifiedRequest;

pub(crate) mod private {
    use std::fmt::Debug;

    use serde::{Serialize, de::DeserializeOwned};

    use super::requests::private::QualifiedRequest;

    pub trait SealedRequest:
        Debug + Clone + Send + Sync + Serialize + DeserializeOwned + Into<QualifiedRequest>
    {
    }
}

/// Request to the hsm server
pub trait Request: private::SealedRequest {
    type Response: Debug + Clone + Serialize + DeserializeOwned;
}

pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_SEQUENTIAL_REQUEST_LEN: usize = 256;
pub const MAX_PARALLEL_REQUEST_LEN: usize = 16;

/// Serialized format for requests to the hsm server
#[derive(Debug, Clone, Serialize, Deserialize)]
enum RequestMessage {
    /// Run a single request
    Single(QualifiedRequest),
    /// Run multiple requests in sequence, abort on the first failure
    Sequential(Vec<QualifiedRequest>),
    /// Run multiple requests in parallel, wait for all to succeed or fail
    Parallel(Vec<QualifiedRequest>),
}

/// Error from the hsm server
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum Error {
    /// The client sent a malformed request
    #[error("Invalid request format: {0}")]
    BadFormat(String),

    /// An unexpected error occured on the server
    #[error("Server Error: {0}")]
    Server(String),
}

/// Response from the hsm server
#[allow(type_alias_bounds)]
pub type Response<R>
where
    R: Request,
= Result<<R as Request>::Response, Error>;

/// Serialized format for responses from the hsm server
type ResponseMessage = Vec<Result<String, Error>>;

macro_rules! make_requests {
(@doc $($doc:literal)* @def $name:ident ()) => {
    $(#[doc = $doc])*
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct $name;
};

(@doc $($doc:literal)* @def $name:ident ( $($field:ty),* )) => {
    $(#[doc = $doc])*
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct $name($(pub $field),*);
};

(@doc $($doc:literal)* @def $name:ident { $($t:tt)* } ) => {
    $(#[doc = $doc])*
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct $name{$($t)*}
};

(
    $($(#[doc = $doc:literal])* $name:ident $fields:tt -> $response:ty;)*
) => {
paste::paste! {
pub(crate) mod private {
    use crate::{requests};
    use super::*;

    /// Prefer using `Request.into()` or generics when writing requests
    /// This type is only needed to distinguish between requests when sending them to the server
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub enum QualifiedRequest {
        $(
        $name(super::$name),
        )*
    }

    pub async fn _handle_single(request: QualifiedRequest, handler: &(impl RequestHandler + ?Sized)) -> Result<String, crate::Error> {
        let reply_data = match request {
        $(
            QualifiedRequest::$name(request) => {
                crate::server::serialize_response::<super::$name>(handler.[<handle_$name:snake>](request).await?)
            }
        )*
        };

        Ok(reply_data)
    }

    pub trait RequestHandler {
        $(
        fn [<handle_$name:snake>](&self, request: requests::$name) -> impl Future<Output = Result<$response, crate::Error>> + Send;
        )*

        fn handle_parallel(&self, requests: Vec<impl Future<Output = Result<String, crate::Error>> + Send>) -> impl Future<Output = Vec<Result<String, crate::Error>>> + Send;
    }
}

    use private::QualifiedRequest;

    $(
    make_requests!(@doc $($doc)* @def $name $fields);

    impl From<$name> for QualifiedRequest {
        fn from(value: $name) -> Self {
            QualifiedRequest::$name(value)
        }
    }

    impl crate::private::SealedRequest for $name {}
    impl crate::Request for $name {
        type Response = $response;
    }
    )*
}
};
}

pub(crate) use make_requests;
