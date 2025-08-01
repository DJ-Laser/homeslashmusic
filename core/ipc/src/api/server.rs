use super::{Reply, client, requests};

pub(crate) mod private {
  use serde::{Deserialize, Serialize};

  use crate::requests;

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub enum QualifiedRequest {
    Version(requests::Version),
    Playback(requests::Playback),
    Set(requests::Set),
    LoadTrack(requests::LoadTrack),
    Seek(requests::Seek),
  }
}

#[allow(async_fn_in_trait)]
pub trait RequestHandler {
  async fn handle_version(&self, request: requests::Version) -> Reply<requests::Version>;
  async fn handle_playback(&self, request: requests::Playback) -> Reply<requests::Playback>;
  async fn handle_set(&self, request: requests::Set) -> Reply<requests::Set>;
  async fn handle_load_track(&self, request: requests::LoadTrack) -> Reply<requests::LoadTrack>;
  async fn handle_seek(&self, request: requests::Seek) -> Reply<requests::Seek>;
}

fn serialize_reply<R: client::Request>(reply: &Reply<R>) -> String {
  let mut reply_data = serde_json::to_string(reply).expect("Replies should not fail to serialize");
  reply_data.push('\n');
  reply_data
}

pub fn serialize_error(error: String) -> String {
  let mut reply_data =
    serde_json::to_string(&Err::<(), String>(error)).expect("Replies should not fail to serialize");
  reply_data.push('\n');
  reply_data
}

pub async fn handle_request(request_data: &str, handler: &impl RequestHandler) -> String {
  let request = match serde_json::from_str(request_data) {
    Ok(request) => request,
    Err(error) => {
      println!("{}", &error);
      return serialize_error(error.to_string());
    }
  };

  match request {
    private::QualifiedRequest::Version(request) => {
      serialize_reply::<requests::Version>(&handler.handle_version(request).await)
    }
    private::QualifiedRequest::Playback(request) => {
      serialize_reply::<requests::Playback>(&handler.handle_playback(request).await)
    }
    private::QualifiedRequest::Set(request) => {
      serialize_reply::<requests::Set>(&handler.handle_set(request).await)
    }
    private::QualifiedRequest::LoadTrack(request) => {
      serialize_reply::<requests::LoadTrack>(&handler.handle_load_track(request).await)
    }
    private::QualifiedRequest::Seek(request) => {
      serialize_reply::<requests::Seek>(&handler.handle_seek(request).await)
    }
  }
}
