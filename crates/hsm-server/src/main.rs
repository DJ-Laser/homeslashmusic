mod audio_server;
mod database;
mod ipc;
mod mpris;
mod signals;

use std::sync::Arc;

use futures_concurrency::future::Race;
use hsm_ipc::{
    Event, Reply, Request,
    client::{deserialize_reply, serialize_request},
};
use log::{error, info};
use smol::{Executor, channel};
use thiserror::Error;

use crate::{
    audio_server::{AudioServer, AudioServerError},
    signals::{SignalHandler, SignalHandlerError},
};
use crate::{database::DatabaseHandle, ipc::IpcServerError, mpris::MprisServerError};

#[derive(Debug, Error)]
pub enum MainError {
    #[error(transparent)]
    AudioServer(#[from] AudioServerError),

    #[error(transparent)]
    SignalHandler(#[from] SignalHandlerError),

    #[error(transparent)]
    Mpris(#[from] MprisServerError),

    #[error(transparent)]
    Ipc(#[from] IpcServerError),
}

pub type RequestJson = (String, async_oneshot::Sender<String>);

#[derive(Debug, Clone)]
pub struct AppHandle {
    executor: Arc<Executor<'static>>,
    database: DatabaseHandle,
    event_rx: async_broadcast::InactiveReceiver<Event>,
    request_data_tx: channel::Sender<RequestJson>,
}

impl AppHandle {
    fn new(
        executor: Arc<Executor<'static>>,
        database: DatabaseHandle,
    ) -> (
        Self,
        (
            channel::Receiver<RequestJson>,
            async_broadcast::Sender<Event>,
        ),
    ) {
        let (request_data_tx, request_data_rx) = channel::unbounded();
        let (mut event_tx, event_rx) = async_broadcast::broadcast(128);

        event_tx.set_overflow(true);

        (
            Self {
                executor,
                database,
                event_rx: event_rx.deactivate(),
                request_data_tx,
            },
            (request_data_rx, event_tx),
        )
    }

    pub async fn send_json(&self, request_data: String) -> String {
        let (reply_tx, reply_rx) = async_oneshot::oneshot();

        if let Err(error) = self.request_data_tx.send((request_data, reply_tx)).await {
            return hsm_ipc::server::serialize_error(&error);
        }

        reply_rx.await.unwrap_or_else(|_| {
            hsm_ipc::server::serialize_error(&"Audio Server dropped reply sender")
        })
    }

    pub fn executor(&self) -> &Arc<Executor<'static>> {
        &self.executor
    }

    async fn send_request<R: Request>(&self, request: R) -> Reply<R>
    where
        Self: Send + Sync,
    {
        let reply_data = self.send_json(serialize_request(request)).await;
        deserialize_reply::<R>(&reply_data).expect("Hsm plugins should not fail json parsing")
    }

    pub fn subscribe(&self) -> async_broadcast::Receiver<Event> {
        self.event_rx.activate_cloned()
    }
}

async fn run_servers(executor: Arc<Executor<'static>>) -> Result<(), MainError> {
    let mut signal_handler = SignalHandler::init()?;

    let databse = database::init_blank_database(&executor);
    let (handle, audio_server_channels) = AppHandle::new(executor, databse);

    let server_futures = (
        async {
            AudioServer::init(audio_server_channels)?
                .run()
                .await
                .map_err(Into::into)
        },
        async {
            mpris::run_mpris_server(handle.clone())
                .await
                .map_err(Into::into)
        },
        async {
            ipc::run_ipc_server(handle.clone())
                .await
                .map_err(Into::into)
        },
        async {
            signal_handler.wait_for_quit().await;
            Ok(())
        },
    );

    server_futures.race().await
}

fn main() {
    env_logger::init();

    let executor: Arc<Executor<'static>> = Arc::new(Executor::new());
    match smol::block_on(executor.run(run_servers(executor.clone()))) {
        Ok(()) => (),
        Err(error) => error!("{error}"),
    }

    info!("hsm-server shutting down");
}
