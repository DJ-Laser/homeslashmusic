use hsm_ipc::{Event, Reply, Request};

pub trait RequestSender {
  fn send_request<R: Request>(&self, request: R) -> impl Future<Output = Reply<R>> + Send + Sync;
}

/// An ipc client that is compiled into the `hsm-server` binary
/// Communication is done via channels instead of json.
pub trait Plugin<Tx: RequestSender> {
  type Error;

  fn init(request_tx: Tx) -> impl Future<Output = Result<Self, Self::Error>> + Send
  where
    Self: Sized;

  fn on_event(&self, event: Event) -> impl Future<Output = Result<(), Self::Error>> + Send;

  fn run(&self) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
