use async_oneshot as oneshot;
use futures_concurrency::future::Race;
use hsm_ipc::Event;
use hsm_plugin::Plugin;
use smol::{
  channel::{self, Receiver, Sender},
  lock::Mutex,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
  #[error("Internal AudioServer Error: Player Event channel closed")]
  EventChannelClosed,

  #[error(transparent)]
  PluginError(Box<dyn std::error::Error>),
}

pub type RequestJson = (String, oneshot::Sender<String>);

#[derive(Debug, Clone)]
pub struct RequestSender {
  request_data_tx: Sender<RequestJson>,
}

impl hsm_plugin::RequestSender for RequestSender {
  async fn send_json(&self, request_data: String) -> String {
    let (reply_tx, reply_rx) = oneshot::oneshot();

    if let Err(error) = self.request_data_tx.send((request_data, reply_tx)).await {
      return hsm_ipc::server::serialize_error(&error);
    }

    reply_rx
      .await
      .unwrap_or_else(|_| hsm_ipc::server::serialize_error(&"Audio Server dropped reply sender"))
  }
}

pub struct PluginRunner<P> {
  plugin: P,
  event_rx: Receiver<Event>,
}

impl<P: Plugin<RequestSender>> PluginRunner<P> {
  fn map_error(error: P::Error) -> PluginError {
    PluginError::PluginError(Box::new(error))
  }

  async fn recieve_events(&self) -> Result<(), PluginError> {
    loop {
      let event = self
        .event_rx
        .recv()
        .await
        .map_err(|_| PluginError::EventChannelClosed)?;

      self.plugin.on_event(event).await.map_err(Self::map_error)?;
    }
  }

  pub async fn run(&self) -> Result<(), PluginError> {
    (
      async { self.plugin.run().await.map_err(Self::map_error) },
      self.recieve_events(),
    )
      .race()
      .await
  }
}

#[derive(Debug)]
pub struct PluginManager {
  request_data_tx: Sender<RequestJson>,

  event_rx: Receiver<Event>,
  event_broadcast_tx: Mutex<Vec<Sender<Event>>>,
}

impl PluginManager {
  pub fn new(request_data_tx: Sender<RequestJson>) -> (Self, Sender<Event>) {
    let (event_tx, event_rx) = channel::unbounded();

    (
      Self {
        request_data_tx,

        event_rx,
        event_broadcast_tx: Mutex::new(Vec::new()),
      },
      event_tx,
    )
  }

  pub fn request_sender(&self) -> RequestSender {
    RequestSender {
      request_data_tx: self.request_data_tx.clone(),
    }
  }

  pub async fn load_plugin<P: Plugin<RequestSender>>(
    &self,
  ) -> Result<PluginRunner<P>, PluginError> {
    let plugin = P::init(self.request_sender())
      .await
      .map_err(PluginRunner::<P>::map_error)?;

    let (event_tx, event_rx) = channel::unbounded();
    self.event_broadcast_tx.lock().await.push(event_tx);

    Ok(PluginRunner { plugin, event_rx })
  }

  async fn broadcast(&self, event: Event) {
    self.event_broadcast_tx.lock().await.retain(|tx| {
      // Remove closed channels
      tx.try_send(event.clone()).is_ok()
    });
  }

  pub async fn run(&self) -> Result<(), PluginError> {
    loop {
      let event = self
        .event_rx
        .recv()
        .await
        .map_err(|_| PluginError::EventChannelClosed)?;

      self.broadcast(event).await;
    }
  }
}
