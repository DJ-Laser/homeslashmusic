mod conversions;
mod mpris_impl;

use futures_concurrency::future::Race as _;
use hsm_ipc::Event;
use mpris_impl::MprisImpl;
use mpris_server::{
    Property, Server, Signal,
    zbus::{self},
};
use smol::channel::{self};
use thiserror::Error;

use crate::{
    AppHandle,
    mpris::conversions::{as_dbus_time, as_loop_status, as_playback_status},
};

#[derive(Debug, Error)]
pub enum MprisServerError {
    #[error("Mpris server error: {0}")]
    DBus(#[from] zbus::Error),

    #[error("Event channel closed")]
    EventChannelClosed,
}

pub const BUS_NAME_SUFFIX: &str = "dev.djlaser.HomeSlashMusic";

async fn on_event(event: Event, server: &Server<MprisImpl>) -> Result<(), MprisServerError> {
    match event {
        Event::PlaybackStateChanged(playback_state) => {
            server
                .properties_changed([Property::PlaybackStatus(as_playback_status(playback_state))])
                .await?;
        }
        Event::LoopModeChanged(loop_mode) => {
            server
                .properties_changed([Property::LoopStatus(as_loop_status(loop_mode))])
                .await?;
        }
        Event::ShuffleChanged(shuffle) => {
            server
                .properties_changed([Property::Shuffle(shuffle)])
                .await?;
        }
        Event::VolumeChanged(volume) => {
            server
                .properties_changed([Property::Volume(volume.into())])
                .await?;
        }
        Event::Seeked(position) => {
            server
                .emit(Signal::Seeked {
                    position: as_dbus_time(position),
                })
                .await?;
        }
    }

    Ok(())
}

pub async fn run_mpris_server(handle: AppHandle) -> Result<(), MprisServerError> {
    let (quit_tx, quit_rx) = channel::bounded(1);
    let mut event_rx = handle.subscribe();

    let server =
        Server::new_with_track_list(BUS_NAME_SUFFIX, MprisImpl::new(handle, quit_tx)).await?;

    let mut quit = false;
    loop {
        (
            async {
                let _ = quit_rx.recv().await;
                quit = true;
            },
            async {
                if let Ok(event) = event_rx.recv().await {
                    let _ = on_event(event, &server).await;
                }
            },
        )
            .race()
            .await;

        if quit {
            break;
        }
    }

    Ok(())
}
