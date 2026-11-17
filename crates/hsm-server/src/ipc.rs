use std::{
    fs,
    mem::DropGuard,
    path::{Path, PathBuf},
    time::Duration,
};

use hsm_ipc::{
    LoopMode, PlaybackState, QueuedTrack, QueuedTrackId, SeekPosition, requests,
    server::RequestHandler,
};
use log::{debug, error};
use smol::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::unix::{UnixListener, UnixStream},
    stream::StreamExt,
};
use thiserror::Error;

use crate::{
    AppHandle,
    audio::{PlayerError, TrackCache},
    ipc::IpcServerError::ListenerClosed,
};

#[derive(Debug, Error)]
pub enum IpcServerError {
    #[error("Failed to check socket file: {0}")]
    CheckSocketFileFailed(#[source] io::Error),

    #[error(
        "The homeslashmusic socket file was already present, is another `hsm-server` instance running?"
    )]
    SocketInUse,

    #[error("Failed to create ipc socket: {0}")]
    FailedToCreateSocket(#[source] io::Error),

    #[error("Listener unexpectedly closed")]
    ListenerClosed,
}

fn is_socket_in_use(socket_path: &Path) -> Result<bool, IpcServerError> {
    let socket_in_use = fs::exists(socket_path).map_err(IpcServerError::CheckSocketFileFailed)?;
    Ok(socket_in_use)
}

fn cleanup_socket(socket_path: &Path) {
    let _ = fs::remove_file(socket_path);
}

pub async fn run_ipc_server(handle: AppHandle) -> Result<(), IpcServerError> {
    let socket_path = PathBuf::from(hsm_ipc::socket_path());
    if is_socket_in_use(&socket_path)? {
        return Err(IpcServerError::SocketInUse);
    }

    let listener = DropGuard::new(
        UnixListener::bind(&socket_path).map_err(IpcServerError::FailedToCreateSocket)?,
        |_| cleanup_socket(&socket_path),
    );

    while let Some(stream) = listener.incoming().next().await {
        let stream_handle = handle.clone();
        handle
            .executor
            .spawn(async {
                let res = if let Ok(stream) = stream {
                    handle_stream(stream_handle, stream).await
                } else {
                    stream.map(|_| ())
                };

                if let Err(error) = res {
                    error!("failed to connect to ipc client: {error}");
                }
            })
            .detach();
    }

    Err(ListenerClosed)
}

async fn handle_stream(handle: AppHandle, stream: UnixStream) -> io::Result<()> {
    let mut request_data = String::new();
    let mut stream_reader = BufReader::new(stream);
    stream_reader.read_line(&mut request_data).await?;

    let reply_data = match hsm_ipc::server::handle_request(&request_data, &handle).await {
        Ok(reply_data) => reply_data,

        Err((reply_data, error)) => {
            error!("{error}");
            reply_data
        }
    };

    let mut stream = stream_reader.into_inner();
    stream.write_all(reply_data.as_bytes()).await?;

    Ok(())
}

impl RequestHandler for AppHandle {
    type Error = PlayerError;

    async fn handle_query_version(
        &self,
        _request: requests::GetVersion,
    ) -> Result<hsm_ipc::Version, Self::Error> {
        Ok(hsm_ipc::version())
    }

    async fn handle_query_playback_state(
        &self,
        _request: requests::GetPlaybackState,
    ) -> Result<PlaybackState, Self::Error> {
        Ok(self.player.playback_state())
    }

    async fn handle_play(&self, _request: requests::Play) -> Result<(), Self::Error> {
        self.player.play().await;
        Ok(())
    }

    async fn handle_pause(&self, _request: requests::Pause) -> Result<(), Self::Error> {
        self.player.pause();
        Ok(())
    }

    async fn handle_stop_playback(
        &self,
        _request: requests::StopPlayback,
    ) -> Result<(), Self::Error> {
        self.player.stop();
        Ok(())
    }

    async fn handle_toggle_playback(
        &self,
        _request: requests::TogglePlayback,
    ) -> Result<(), Self::Error> {
        self.player.toggle_playback().await;
        Ok(())
    }

    async fn handle_query_current_track(
        &self,
        _request: requests::GetCurrentQueuedTrack,
    ) -> Result<Option<QueuedTrack>, Self::Error> {
        let current_track = self.player.current_track();
        let current_track = current_track.map(|track| track.clone_owned_instance());

        Ok(current_track)
    }

    async fn handle_query_current_track_id(
        &self,
        _request: requests::GetCurrentQueuedTrackId,
    ) -> Result<Option<QueuedTrackId>, Self::Error> {
        let current_track = self.player.current_track();
        let current_track_id = current_track.map(|track| track.id);

        Ok(current_track_id)
    }

    async fn handle_go_to_track(
        &self,
        requests::GoToTrack(track_id): requests::GoToTrack,
    ) -> Result<bool, Self::Error> {
        Ok(self.player.go_to_track(track_id).await)
    }

    async fn handle_go_to_queue_index(
        &self,
        requests::GoToQueueIndex(index): requests::GoToQueueIndex,
    ) -> Result<bool, Self::Error> {
        Ok(self.player.go_to_index(index).await)
    }

    async fn handle_go_to_next_track(
        &self,
        _request: requests::GoToNextTrack,
    ) -> Result<(), Self::Error> {
        self.player.go_to_next_track().await
    }

    async fn handle_go_to_previous_track(
        &self,
        requests::GoToPreviousTrack { soft }: requests::GoToPreviousTrack,
    ) -> Result<(), Self::Error> {
        self.player.go_to_previous_track(soft).await
    }

    async fn handle_query_loop_mode(
        &self,
        _request: requests::GetLoopMode,
    ) -> Result<LoopMode, Self::Error> {
        Ok(self.player.loop_mode())
    }

    async fn handle_set_loop_mode(
        &self,
        requests::SetLoopMode(loop_mode): requests::SetLoopMode,
    ) -> Result<(), Self::Error> {
        self.player.set_loop_mode(loop_mode);
        Ok(())
    }

    async fn handle_query_shuffle(
        &self,
        _request: requests::GetShuffle,
    ) -> Result<bool, Self::Error> {
        Ok(self.player.shuffle())
    }

    async fn handle_set_shuffle(
        &self,
        requests::SetShuffle(shuffle): requests::SetShuffle,
    ) -> Result<(), Self::Error> {
        self.player.set_shuffle(shuffle).await;
        Ok(())
    }

    async fn handle_query_volume(&self, _request: requests::GetVolume) -> Result<f32, Self::Error> {
        Ok(self.player.volume())
    }

    async fn handle_set_volume(
        &self,
        requests::SetVolume(volume): requests::SetVolume,
    ) -> Result<(), Self::Error> {
        self.player.set_volume(volume);
        Ok(())
    }

    async fn handle_query_position(
        &self,
        _request: requests::GetPosition,
    ) -> Result<Duration, Self::Error> {
        Ok(self.player.position())
    }

    async fn handle_seek_current(
        &self,
        requests::SeekCurrent(seek_offset): requests::SeekCurrent,
    ) -> Result<(), Self::Error> {
        self.player.seek(seek_offset.into()).await?;
        Ok(())
    }

    async fn handle_seek_to(
        &self,
        requests::SeekTo(track_id, position): requests::SeekTo,
    ) -> Result<bool, Self::Error> {
        if let Some(track) = self.player.current_track()
            && track.id == track_id
        {
            self.player.seek(SeekPosition::To(position)).await?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn handle_query_queue_tracks(
        &self,
        _request: requests::GetQueuedTracks,
    ) -> Result<Vec<QueuedTrack>, Self::Error> {
        let tracks = self.player.get_tracks();
        let tracks = tracks
            .into_iter()
            .map(|track| track.clone_owned_instance())
            .collect();

        Ok(tracks)
    }

    async fn handle_query_queue_track_ids(
        &self,
        _request: requests::GetQueuedTrackIds,
    ) -> Result<Vec<QueuedTrackId>, Self::Error> {
        let tracks = self.player.get_tracks();
        let track_ids = tracks.into_iter().map(|track| track.id).collect();

        Ok(track_ids)
    }

    async fn handle_clear_tracks(&self, _request: requests::ClearQueue) -> Result<(), Self::Error> {
        self.player.clear_tracks();
        Ok(())
    }

    async fn handle_load_tracks(
        &self,
        requests::LoadTracks {
            position,
            paths,
            set_as_current,
        }: requests::LoadTracks,
    ) -> Result<Vec<(PathBuf, String)>, Self::Error> {
        let track_cache = TrackCache::new();

        debug!("Loading tracks: {paths:?}");
        let (tracks, errors) = track_cache.get_or_load_tracks(paths).await;

        for (path, error) in &errors {
            error!("Could not load track {}: {error}", path.display());
        }

        for track in &tracks {
            debug!("Loaded track {:?}", track.track_id);
        }

        (self
            .player
            .insert_tracks(position, &tracks, set_as_current)
            .await)?;

        Ok(errors
            .into_iter()
            .map(|(path, error)| (path, error.to_string()))
            .collect())
    }
}
