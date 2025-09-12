use std::error::Error;

use hsm_ipc::{
  Event, Reply, Request,
  client::{deserialize_reply, serialize_request},
};

async fn send_request<R: Request>(sender: &(impl RequestSender + ?Sized), request: R) -> Reply<R> {
  let reply_data = sender.send_json(serialize_request(request)).await;
  deserialize_reply::<R>(&reply_data).expect("Hsm plugins should not fail json parsing")
}

pub trait RequestSender {
  fn send_json(&self, request_data: String) -> impl Future<Output = String> + Send + Sync;

  fn send_request<R: Request>(&self, request: R) -> impl Future<Output = Reply<R>> + Send + Sync
  where
    Self: Send + Sync,
  {
    send_request(self, request)
  }
}

/// An ipc client that is compiled into the `hsm-server` binary
/// Communication is done via channels instead of json.
pub trait Plugin<Tx: RequestSender> {
  type Error: Error + 'static;

  fn init(request_tx: Tx) -> impl Future<Output = Result<Self, Self::Error>> + Send
  where
    Self: Sized;

  fn on_event(&self, event: Event) -> impl Future<Output = Result<(), Self::Error>> + Send;

  fn run(&self) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
