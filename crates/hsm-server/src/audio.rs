mod controlled_source;
mod decoder;
mod output;
mod player;
mod track_list;

use std::sync::Arc;

use hsm_ipc::Event;
use log::info;
use smol::channel;
use thiserror::Error;

pub use player::{Player, PlayerError};
pub use track_list::InsertTrackData;

#[derive(Debug, Error)]
pub enum AudioServerError {
    #[error(transparent)]
    StreamError(#[from] rodio::StreamError),

    #[error("Error initializing Player")]
    InitError,
}

pub async fn start_audio_server(
    event_tx: async_broadcast::Sender<Event>,
) -> Result<Arc<Player>, AudioServerError> {
    let (mut init_tx, init_rx) = async_oneshot::oneshot();

    std::thread::spawn(move || {
        let mut output_stream = match rodio::OutputStreamBuilder::open_default_stream() {
            Ok(s) => s,
            Err(e) => {
                let _ = init_tx.send(Err(AudioServerError::from(e)));
                return;
            }
        };

        let (guard_tx, guard_rx) = channel::bounded(1);
        let player = Arc::new(Player::connect_new(
            event_tx,
            output_stream.mixer(),
            guard_tx,
        ));

        let _ = init_tx.send(Ok(player));

        // Block this thread until Player is dropped, then drop output_stream
        output_stream.log_on_drop(false);
        let _ = guard_rx.recv_blocking();
        info!("Dropping audio output stream, all audio will stop");
    });

    init_rx.await.map_err(|_| AudioServerError::InitError)?
}
