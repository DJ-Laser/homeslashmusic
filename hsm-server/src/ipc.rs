use std::{
  fs,
  path::{Path, PathBuf},
  sync::Arc,
};

use hsm_plugin::RequestSender;
use smol::{
  Executor,
  io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::unix::{UnixListener, UnixStream},
  stream::StreamExt,
};
use thiserror::Error;

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

pub struct IpcServer<'ex, Tx> {
  socket_path: PathBuf,
  request_sender: Tx,
  ex: Arc<Executor<'ex>>,
}

impl<'ex, Tx> IpcServer<'ex, Tx> {
  fn is_socket_in_use(socket_path: &Path) -> Result<bool, IpcServerError> {
    let socket_in_use = fs::exists(socket_path).map_err(IpcServerError::CheckSocketFileFailed)?;
    Ok(socket_in_use)
  }

  pub fn new(request_sender: Tx, ex: Arc<Executor<'ex>>) -> Result<Self, IpcServerError> {
    let socket_path = PathBuf::from(hsm_ipc::socket_path());
    if Self::is_socket_in_use(&socket_path)? {
      return Err(IpcServerError::SocketInUse);
    }

    Ok(Self {
      socket_path,
      request_sender,
      ex,
    })
  }

  fn cleanup_socket(&self) {
    let _ = fs::remove_file(&self.socket_path);
    println!("Removing socket: {:?}", self.socket_path);
  }
}

impl<'ex, Tx: RequestSender + Send + Sync + Clone + 'ex> IpcServer<'ex, Tx> {
  pub async fn run(&self) -> Result<(), IpcServerError> {
    let listener =
      UnixListener::bind(&self.socket_path).map_err(IpcServerError::FailedToCreateSocket)?;

    while let Some(stream) = listener.incoming().next().await {
      let request_sender = self.request_sender.clone();

      self
        .ex
        .spawn(async {
          let res = if let Ok(stream) = stream {
            StreamHandler::new(request_sender)
              .handle_stream(stream)
              .await
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
}

impl<'ex, Tx> Drop for IpcServer<'ex, Tx> {
  fn drop(&mut self) {
    self.cleanup_socket();
  }
}

struct StreamHandler<Tx> {
  request_sender: Tx,
}

impl<Tx> StreamHandler<Tx> {
  fn new(request_sender: Tx) -> Self {
    Self { request_sender }
  }
}

impl<Tx: RequestSender> StreamHandler<Tx> {
  async fn handle_stream(&self, stream: UnixStream) -> io::Result<()> {
    let mut request_data = String::new();
    let mut stream_reader = BufReader::new(stream);
    stream_reader.read_line(&mut request_data).await?;

    let reply_data = self.request_sender.send_json(request_data).await;

    let mut stream = stream_reader.into_inner();
    stream.write_all(&reply_data.as_bytes()).await?;

    Ok(())
  }
}
