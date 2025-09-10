use super::{QualifiedRequest, Reply, Request};

pub fn serialize_request(request: impl Request) -> String {
  let mut request_data = serde_json::to_string::<QualifiedRequest>(&request.into())
    .expect("Requests should not fail to serialize");
  request_data.push('\n');
  request_data
}

pub fn deserialize_reply<R: Request>(reply_data: &str) -> serde_json::Result<Reply<R>> {
  serde_json::from_str(reply_data)
}
