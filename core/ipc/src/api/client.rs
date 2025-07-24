use std::fmt::Debug;

use serde::{Serialize, de::DeserializeOwned};

use super::{Reply, server};

pub(crate) mod private {

  use super::*;
  pub trait SealedRequest: Debug + Clone + Serialize + DeserializeOwned {
    fn qualified_request(self) -> server::private::QualifiedRequest;
  }
}

/// Request sent to the hsm server
pub trait Request: private::SealedRequest {
  type Response: Debug + Clone + Serialize + DeserializeOwned;
}

pub fn serialize_request(request: impl Request) -> String {
  let mut request_data = serde_json::to_string(&request.qualified_request())
    .expect("Requests should not fail to serialize");
  request_data.push('\n');
  request_data
}

pub fn deserialize_reply<R: Request>(reply_data: &str) -> serde_json::Result<Reply<R>> {
  serde_json::from_str(reply_data)
}
