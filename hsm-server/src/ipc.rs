use std::{
  fs,
  path::{Path, PathBuf},
  sync::Arc,
};

use async_oneshot as oneshot;
use hsm_ipc::{
  Reply, requests,
  server::{RequestHandler, handle_request},
};
use smol::{
  Executor,
  channel::Sender,
  io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::unix::{UnixListener, UnixStream},
  stream::StreamExt,
};
use thiserror::Error;

use crate::audio_server::message::{Message, Query};

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
}

pub struct IpcServer<'ex> {
  socket_path: PathBuf,
  message_tx: Sender<Message>,
  ex: Arc<Executor<'ex>>,
}

impl<'ex> IpcServer<'ex> {
  fn is_socket_in_use(socket_path: &Path) -> Result<bool, IpcServerError> {
    let socket_in_use = fs::exists(socket_path).map_err(IpcServerError::CheckSocketFileFailed)?;
    Ok(socket_in_use)
  }

  pub fn new(message_tx: Sender<Message>, ex: Arc<Executor<'ex>>) -> Result<Self, IpcServerError> {
    let socket_path = PathBuf::from(hsm_ipc::socket_path());
    if Self::is_socket_in_use(&socket_path)? {
      return Err(IpcServerError::SocketInUse);
    }

    Ok(Self {
      socket_path,
      message_tx,
      ex,
    })
  }

  pub async fn run(&self) -> Result<(), IpcServerError> {
    let listener =
      UnixListener::bind(&self.socket_path).map_err(IpcServerError::FailedToCreateSocket)?;

    while let Some(stream) = listener.incoming().next().await {
      let message_tx = self.message_tx.clone();

      self
        .ex
        .spawn(async {
          let res = if let Ok(stream) = stream {
            StreamHandler::new(message_tx).handle_stream(stream).await
          } else {
            stream.map(|_| ())
          };

          if let Err(error) = res {
            eprintln!("failed to connect to ipc client: {}", error);
          }
        })
        .detach();
    }

    self.cleanup_socket();
    unreachable!("Iterating over Incoming should never return None")
  }

  fn cleanup_socket(&self) {
    let _ = fs::remove_file(&self.socket_path);
    println!("Removing socket: {:?}", self.socket_path);
  }
}

impl<'ex> Drop for IpcServer<'ex> {
  fn drop(&mut self) {
    self.cleanup_socket();
  }
}

struct StreamHandler {
  message_tx: Sender<Message>,
}

impl StreamHandler {
  fn new(message_tx: Sender<Message>) -> Self {
    Self { message_tx }
  }

  fn oneshot_closed_error<T>(_t: T) -> String {
    "Channel was unexpectedly closed".into()
  }

  async fn handle_stream(&self, stream: UnixStream) -> io::Result<()> {
    let mut request_data = String::new();
    let mut stream_reader = BufReader::new(stream);
    stream_reader.read_line(&mut request_data).await?;

    let reply_data = handle_request(&request_data, self).await;

    let mut stream = stream_reader.into_inner();
    stream.write_all(&reply_data.as_bytes()).await?;

    Ok(())
  }

  async fn try_send_message(&self, message: Message) -> Result<(), String> {
    self
      .message_tx
      .send(message)
      .await
      .map_err(|e| e.to_string())
  }

  async fn try_query<T>(&self, query: impl Fn(oneshot::Sender<T>) -> Query) -> Result<T, String> {
    let (query_tx, query_rx) = oneshot::oneshot();
    self
      .try_send_message(Message::Query(query(query_tx)))
      .await?;
    Ok(
      query_rx
        .await
        .map_err(|_| "sending into a closed channel")?,
    )
  }
}

impl RequestHandler for StreamHandler {
  async fn handle_query_version(
    &self,
    _request: requests::QueryVersion,
  ) -> Reply<requests::QueryVersion> {
    Ok(hsm_ipc::version())
  }

  async fn handle_query_playback_state(
    &self,
    _request: requests::QueryPlaybackState,
  ) -> Reply<requests::QueryPlaybackState> {
    self.try_query(Query::PlaybackState).await
  }

  async fn handle_play(&self, _request: requests::Play) -> Reply<requests::Play> {
    self.try_send_message(Message::Play).await
  }

  async fn handle_pause(&self, _request: requests::Pause) -> Reply<requests::Pause> {
    self.try_send_message(Message::Pause).await
  }

  async fn handle_stop_playback(
    &self,
    _request: requests::StopPlayback,
  ) -> Reply<requests::StopPlayback> {
    self.try_send_message(Message::Stop).await
  }

  async fn handle_toggle_playback(
    &self,
    _request: requests::TogglePlayback,
  ) -> Reply<requests::TogglePlayback> {
    self.try_send_message(Message::Toggle).await
  }

  async fn handle_query_current_track(
    &self,
    _request: requests::QueryCurrentTrack,
  ) -> Reply<requests::QueryCurrentTrack> {
    let track = self.try_query(Query::CurrentTrack).await?;

    Ok(track)
  }

  async fn handle_query_current_track_index(
    &self,
    _request: requests::QueryCurrentTrackIndex,
  ) -> Reply<requests::QueryCurrentTrackIndex> {
    self.try_query(Query::CurrentTrackIndex).await
  }

  async fn handle_next_track(&self, _request: requests::NextTrack) -> Reply<requests::NextTrack> {
    self.try_send_message(Message::NextTrack).await
  }

  async fn handle_previous_track(
    &self,
    request: requests::PreviousTrack,
  ) -> Reply<requests::PreviousTrack> {
    self
      .try_send_message(Message::PreviousTrack { soft: request.soft })
      .await
  }

  async fn handle_query_loop_mode(
    &self,
    _request: requests::QueryLoopMode,
  ) -> Reply<requests::QueryLoopMode> {
    self.try_query(Query::LoopMode).await
  }

  async fn handle_set_loop_mode(
    &self,
    request: requests::SetLoopMode,
  ) -> Reply<requests::SetLoopMode> {
    self.try_send_message(Message::SetLoopMode(request.0)).await
  }

  async fn handle_query_shuffle(
    &self,
    _request: requests::QueryShuffle,
  ) -> Reply<requests::QueryShuffle> {
    self.try_query(Query::Shuffle).await
  }

  async fn handle_set_shuffle(&self, request: requests::SetShuffle) -> Reply<requests::SetVolume> {
    self.try_send_message(Message::SetShuffle(request.0)).await
  }

  async fn handle_query_volume(
    &self,
    _request: requests::QueryVolume,
  ) -> Reply<requests::QueryVolume> {
    self.try_query(Query::Volume).await
  }

  async fn handle_set_volume(&self, request: requests::SetVolume) -> Reply<requests::SetVolume> {
    self.try_send_message(Message::SetVolume(request.0)).await
  }

  async fn handle_query_position(
    &self,
    _request: requests::QueryPosition,
  ) -> Reply<requests::QueryPosition> {
    self.try_query(Query::Position).await
  }

  async fn handle_seek(&self, request: requests::Seek) -> Reply<requests::Seek> {
    self.try_send_message(Message::Seek(request.0)).await
  }

  async fn handle_query_track_list(
    &self,
    _request: requests::QueryTrackList,
  ) -> Reply<requests::QueryTrackList> {
    self.try_query(Query::IpcTrackList).await
  }

  async fn handle_clear_tracks(
    &self,
    _request: requests::ClearTracks,
  ) -> Reply<requests::ClearTracks> {
    self.try_send_message(Message::ClearTracks).await
  }

  async fn handle_load_tracks(&self, request: requests::LoadTracks) -> Reply<requests::LoadTracks> {
    let (tx, rx) = oneshot::oneshot();
    self
      .try_send_message(Message::InsertTracks {
        position: request.0,
        paths: request.1,
        error_tx: tx,
      })
      .await?;

    let errors = rx.await.map_err(Self::oneshot_closed_error)?;
    Ok(
      errors
        .into_iter()
        .map(|(path, error)| (path, error.to_string()))
        .collect(),
    )
  }
}
