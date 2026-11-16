use std::{error::Error, sync::Arc};

use async_oneshot as oneshot;
use futures_concurrency::future::Race;
use hsm_ipc::{
    Event, Reply, Request,
    client::{deserialize_reply, serialize_request},
};
use smol::{
    Executor,
    channel::{self, Receiver, Sender},
    lock::Mutex,
};
use thiserror::Error;

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
pub trait Plugin<'ex, Tx: RequestSender> {
    type Error: Error + 'static;

    fn init(
        request_tx: Tx,
        executor: Arc<Executor<'ex>>,
    ) -> impl Future<Output = Result<Self, Self::Error>> + Send
    where
        Self: Sized;

    fn on_event(&self, event: Event) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn run(&self) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Internal AudioServer Error: Player Event channel closed")]
    EventChannelClosed,

    #[error(transparent)]
    PluginError(Box<dyn std::error::Error>),
}

pub type RequestJson = (String, oneshot::Sender<String>);

#[derive(Debug, Clone)]
pub struct RequestSenderChannel {
    request_data_tx: Sender<RequestJson>,
}

impl RequestSender for RequestSenderChannel {
    async fn send_json(&self, request_data: String) -> String {
        let (reply_tx, reply_rx) = oneshot::oneshot();

        if let Err(error) = self.request_data_tx.send((request_data, reply_tx)).await {
            return hsm_ipc::server::serialize_error(&error);
        }

        reply_rx.await.unwrap_or_else(|_| {
            hsm_ipc::server::serialize_error(&"Audio Server dropped reply sender")
        })
    }
}

pub struct PluginRunner<P> {
    plugin: P,
    event_rx: Receiver<Event>,
}

impl<'ex, P: Plugin<'ex, RequestSenderChannel>> PluginRunner<P> {
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
pub struct PluginManager<'ex> {
    executor: Arc<Executor<'ex>>,

    request_data_tx: Sender<RequestJson>,

    event_rx: Receiver<Event>,
    event_broadcast_tx: Mutex<Vec<Sender<Event>>>,
}

impl<'ex> PluginManager<'ex> {
    pub fn new(executor: Arc<Executor<'ex>>) -> (Self, (Receiver<RequestJson>, Sender<Event>)) {
        let (request_data_tx, request_data_rx) = channel::unbounded();
        let (event_tx, event_rx) = channel::unbounded();

        (
            Self {
                executor,
                request_data_tx,

                event_rx,
                event_broadcast_tx: Mutex::new(Vec::new()),
            },
            (request_data_rx, event_tx),
        )
    }

    pub fn request_sender(&self) -> RequestSenderChannel {
        RequestSenderChannel {
            request_data_tx: self.request_data_tx.clone(),
        }
    }

    pub async fn load_plugin<P: Plugin<'ex, RequestSenderChannel>>(
        &self,
    ) -> Result<PluginRunner<P>, PluginError> {
        let plugin = P::init(self.request_sender(), self.executor.clone())
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
