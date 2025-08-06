use std::{fs, path::PathBuf, sync::Arc};

use async_oneshot as oneshot;
use hsm_ipc::{
  Reply, requests, responses,
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

use crate::audio_server::message::Message;

#[derive(Debug, Error)]
pub enum IpcServerError {
  #[error("Failed to create ipc socket at {path}")]
  FailedToCreateSocket { path: PathBuf, source: io::Error },
}

pub struct IpcServer<'ex> {
  socket_path: PathBuf,
  message_tx: Sender<Message>,
  ex: Arc<Executor<'ex>>,
}

impl<'ex> IpcServer<'ex> {
  pub fn new(message_tx: Sender<Message>, ex: Arc<Executor<'ex>>) -> Self {
    Self {
      socket_path: PathBuf::from(hsm_ipc::socket_path()),
      message_tx,
      ex,
    }
  }

  pub async fn run(&self) -> Result<(), IpcServerError> {
    let listener = UnixListener::bind(&self.socket_path).map_err(|source| {
      IpcServerError::FailedToCreateSocket {
        path: self.socket_path.clone(),
        source,
      }
    })?;

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

    unreachable!("Iterating over Incoming should never return None")
  }
}

impl<'ex> Drop for IpcServer<'ex> {
  fn drop(&mut self) {
    let _ = fs::remove_file(&self.socket_path);
    println!("Removing socket: {:?}", self.socket_path)
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
}

impl RequestHandler for StreamHandler {
  async fn handle_version(&self, _: requests::Version) -> Reply<requests::Version> {
    Ok(responses::Version(hsm_ipc::version()))
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

  async fn handle_set_loop_mode(
    &self,
    request: requests::SetLoopMode,
  ) -> Reply<requests::SetLoopMode> {
    self.try_send_message(Message::SetLoopMode(request.0)).await
  }

  async fn handle_set_shuffle(&self, request: requests::SetShuffle) -> Reply<requests::SetVolume> {
    self.try_send_message(Message::SetShuffle(request.0)).await
  }

  async fn handle_set_volume(&self, request: requests::SetVolume) -> Reply<requests::SetVolume> {
    self.try_send_message(Message::SetVolume(request.0)).await
  }

  async fn handle_seek(&self, request: requests::Seek) -> Reply<requests::Seek> {
    self.try_send_message(Message::Seek(request.0)).await
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
