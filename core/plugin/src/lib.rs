use hsm_ipc::{Reply, Request};

pub trait RequestFacilitator {
  fn send_request<R: Request>(request: R) -> impl Future<Output = Reply<R>>;
}

/// An ipc client that is compiled into the `hsm-server` binary
/// Communication is done via channels instead of json.
pub trait Plugin {
  type Error;

  fn init(
    request_tx: impl RequestFacilitator,
  ) -> impl Future<Output = Result<Self, Self::Error>> + Send
  where
    Self: Sized;

  fn on_event(&self) -> impl Future<Output = Result<(), Self::Error>> + Send;

  fn run(&self) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
