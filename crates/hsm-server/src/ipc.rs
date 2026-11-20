use std::{
    fs,
    mem::DropGuard,
    path::{Path, PathBuf},
    time::Duration,
};

use futures_concurrency::concurrent_stream::{ConcurrentStream, IntoConcurrentStream};
use hsm_ipc::{
    LoopMode, PlaybackState, QueuedTrack, QueuedTrackId, SearchResult, SeekPosition, requests,
    server::RequestHandler,
};
use log::error;
use smol::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::unix::{UnixListener, UnixStream},
    stream::StreamExt,
};
use thiserror::Error;

use crate::{AppHandle, audio::PlayerError, database, ipc::IpcServerError::ListenerClosed};

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

    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent).map_err(IpcServerError::FailedToCreateSocket)?;
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

    let reply_data = hsm_ipc::server::handle_request(&request_data, &handle).await;

    let mut stream = stream_reader.into_inner();
    stream.write_all(reply_data.as_bytes()).await?;

    Ok(())
}

impl From<PlayerError> for hsm_ipc::Error {
    fn from(value: PlayerError) -> Self {
        hsm_ipc::Error::Server(value.to_string())
    }
}

impl RequestHandler for AppHandle {
    async fn handle_parallel(
        &self,
        requests: Vec<impl Future<Output = Result<String, hsm_ipc::Error>> + Send>,
    ) -> Vec<Result<String, hsm_ipc::Error>> {
        requests
            .into_co_stream()
            .map(async |f| f.await)
            .collect()
            .await
    }

    async fn handle_get_version(
        &self,
        _request: requests::GetVersion,
    ) -> Result<hsm_ipc::Version, hsm_ipc::Error> {
        Ok(hsm_ipc::version())
    }

    async fn handle_get_playback_state(
        &self,
        _request: requests::GetPlaybackState,
    ) -> Result<PlaybackState, hsm_ipc::Error> {
        Ok(self.player.playback_state())
    }

    async fn handle_play(&self, _request: requests::Play) -> Result<(), hsm_ipc::Error> {
        self.player.play().await;
        Ok(())
    }

    async fn handle_pause(&self, _request: requests::Pause) -> Result<(), hsm_ipc::Error> {
        self.player.pause();
        Ok(())
    }

    async fn handle_stop_playback(
        &self,
        _request: requests::StopPlayback,
    ) -> Result<(), hsm_ipc::Error> {
        self.player.stop();
        Ok(())
    }

    async fn handle_toggle_playback(
        &self,
        _request: requests::TogglePlayback,
    ) -> Result<(), hsm_ipc::Error> {
        self.player.toggle_playback().await;
        Ok(())
    }

    async fn handle_get_current_queued_track(
        &self,
        _request: requests::GetCurrentQueuedTrack,
    ) -> Result<Option<QueuedTrack>, hsm_ipc::Error> {
        let current_track = self.player.current_track();
        let current_track = current_track.map(|track| track.clone_owned_instance());

        Ok(current_track)
    }

    async fn handle_get_current_queued_track_id(
        &self,
        _request: requests::GetCurrentQueuedTrackId,
    ) -> Result<Option<QueuedTrackId>, hsm_ipc::Error> {
        let current_track = self.player.current_track();
        let current_track_id = current_track.map(|track| track.id);

        Ok(current_track_id)
    }

    async fn handle_go_to_track(
        &self,
        requests::GoToTrack(track_id): requests::GoToTrack,
    ) -> Result<bool, hsm_ipc::Error> {
        Ok(self.player.go_to_track(track_id).await)
    }

    async fn handle_go_to_queue_index(
        &self,
        requests::GoToQueueIndex(index): requests::GoToQueueIndex,
    ) -> Result<bool, hsm_ipc::Error> {
        Ok(self.player.go_to_index(index).await)
    }

    async fn handle_go_to_next_track(
        &self,
        _request: requests::GoToNextTrack,
    ) -> Result<(), hsm_ipc::Error> {
        self.player.go_to_next_track().await?;
        Ok(())
    }

    async fn handle_go_to_previous_track(
        &self,
        _request: requests::GoToPreviousTrack,
    ) -> Result<(), hsm_ipc::Error> {
        self.player.go_to_previous_track().await?;
        Ok(())
    }

    async fn handle_get_loop_mode(
        &self,
        _request: requests::GetLoopMode,
    ) -> Result<LoopMode, hsm_ipc::Error> {
        Ok(self.player.loop_mode())
    }

    async fn handle_set_loop_mode(
        &self,
        requests::SetLoopMode(loop_mode): requests::SetLoopMode,
    ) -> Result<(), hsm_ipc::Error> {
        self.player.set_loop_mode(loop_mode);
        Ok(())
    }

    async fn handle_get_shuffle(
        &self,
        _request: requests::GetShuffle,
    ) -> Result<bool, hsm_ipc::Error> {
        Ok(self.player.shuffle())
    }

    async fn handle_set_shuffle(
        &self,
        requests::SetShuffle(shuffle): requests::SetShuffle,
    ) -> Result<(), hsm_ipc::Error> {
        self.player.set_shuffle(shuffle).await;
        Ok(())
    }

    async fn handle_get_volume(
        &self,
        _request: requests::GetVolume,
    ) -> Result<f32, hsm_ipc::Error> {
        Ok(self.player.volume())
    }

    async fn handle_set_volume(
        &self,
        requests::SetVolume(volume): requests::SetVolume,
    ) -> Result<(), hsm_ipc::Error> {
        self.player.set_volume(volume);
        Ok(())
    }

    async fn handle_get_position(
        &self,
        _request: requests::GetPosition,
    ) -> Result<Duration, hsm_ipc::Error> {
        Ok(self.player.position())
    }

    async fn handle_seek_current(
        &self,
        requests::SeekCurrent(seek_offset): requests::SeekCurrent,
    ) -> Result<(), hsm_ipc::Error> {
        self.player.seek(seek_offset.into()).await?;
        Ok(())
    }

    async fn handle_seek_to(
        &self,
        requests::SeekTo(track_id, position): requests::SeekTo,
    ) -> Result<bool, hsm_ipc::Error> {
        if let Some(track) = self.player.current_track()
            && track.id == track_id
        {
            self.player.seek(SeekPosition::To(position)).await?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn handle_get_queued_tracks(
        &self,
        _request: requests::GetQueuedTracks,
    ) -> Result<Vec<QueuedTrack>, hsm_ipc::Error> {
        let tracks = self.player.get_tracks();
        let tracks = tracks
            .into_iter()
            .map(|track| track.clone_owned_instance())
            .collect();

        Ok(tracks)
    }

    async fn handle_get_queued_track_ids(
        &self,
        _request: requests::GetQueuedTrackIds,
    ) -> Result<Vec<QueuedTrackId>, hsm_ipc::Error> {
        let tracks = self.player.get_tracks();
        let track_ids = tracks.into_iter().map(|track| track.id).collect();

        Ok(track_ids)
    }

    async fn handle_clear_queue(
        &self,
        _request: requests::ClearQueue,
    ) -> Result<(), hsm_ipc::Error> {
        self.player.clear_tracks();
        Ok(())
    }

    async fn handle_load_tracks(
        &self,
        requests::LoadTracks(item, position): requests::LoadTracks,
    ) -> Result<QueuedTrackId, hsm_ipc::Error> {
        let tracks = self
            .database
            .read_db(|db| db.queue_insertion(&item))
            .map_err(|error| hsm_ipc::Error::Server(error.to_string()))?;

        let queued_id = self.player.insert_tracks(position, tracks, false).await?;

        queued_id.ok_or(hsm_ipc::Error::Server("Failed to insert any tracks".into()))
    }

    async fn handle_update_database(
        &self,
        request: requests::UpdateDatabase,
    ) -> Result<(), hsm_ipc::Error> {
        self.database
            .run_job(&self.executor, |ctx| database::start_update(ctx, request.0))
            .detach();

        Ok(())
    }

    async fn handle_search(
        &self,
        requests::Search(search_query): requests::Search,
    ) -> Result<Vec<(SearchResult, u32)>, hsm_ipc::Error> {
        let search_result = self
            .database
            .run_job(&self.executor, |ctx| {
                database::run_search(ctx, search_query)
            })
            .await
            .map_err(|error| hsm_ipc::Error::Server(error.to_string()))?;

        Ok(search_result
            .into_iter()
            .map(|(item, score)| (item.into(), score))
            .collect())
    }
}
