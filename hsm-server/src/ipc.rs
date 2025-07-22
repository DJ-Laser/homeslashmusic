use std::{fs, path::PathBuf, sync::Arc};

use hsm_ipc::{
  Reply,
  requests::{self, Playback},
  responses,
  server::{RequestHandler, handle_request},
};
use smol::{
  Executor,
  channel::Sender,
  io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::unix::{UnixListener, UnixStream},
  stream::StreamExt,
};

use crate::audio_server::message;

pub struct IpcServer<'ex> {
  socket_path: PathBuf,
  message_tx: Sender<message::Message>,
  ex: Arc<Executor<'ex>>,
}

impl<'ex> IpcServer<'ex> {
  pub fn new(message_tx: Sender<message::Message>, ex: Arc<Executor<'ex>>) -> Self {
    Self {
      socket_path: PathBuf::from(hsm_ipc::socket_path()),
      message_tx,
      ex,
    }
  }

  pub async fn run(&self) -> Result<(), io::Error> {
    let listener = UnixListener::bind(&self.socket_path)?;

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

    unreachable!("Iterating over `Incoming` should never return `None`")
  }
}

impl<'ex> Drop for IpcServer<'ex> {
  fn drop(&mut self) {
    let _ = fs::remove_file(&self.socket_path);
    println!("Removing socket: `{:?}`", self.socket_path)
  }
}

struct StreamHandler {
  message_tx: Sender<message::Message>,
}

impl StreamHandler {
  fn new(message_tx: Sender<message::Message>) -> Self {
    Self { message_tx }
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
}

impl RequestHandler for StreamHandler {
  async fn handle_version(&self, _: requests::Version) -> Reply<requests::Version> {
    Ok(responses::Version(hsm_ipc::version()))
  }

  async fn handle_playback(&self, request: requests::Playback) -> Reply<requests::Playback> {
    use crate::audio_server::message::{Message, PlaybackControl};

    let message = match request {
      Playback::Play => PlaybackControl::Play,
      Playback::Pause => PlaybackControl::Pause,
      Playback::Toggle => PlaybackControl::Toggle,
    };

    self
      .message_tx
      .send(Message::Playback(message))
      .await
      .map_err(|e| e.to_string())
      .map(|_| responses::Handled)
  }
}
