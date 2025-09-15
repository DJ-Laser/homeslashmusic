use std::{
  fs,
  path::{Path, PathBuf},
  sync::Arc,
};

use hsm_plugin::{Plugin, RequestSender};
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

pub struct IpcPlugin<'ex, Tx> {
  socket_path: PathBuf,
  request_tx: Tx,
  executor: Arc<Executor<'ex>>,
}

impl<'ex, Tx> IpcPlugin<'ex, Tx> {
  fn is_socket_in_use(socket_path: &Path) -> Result<bool, IpcServerError> {
    let socket_in_use = fs::exists(socket_path).map_err(IpcServerError::CheckSocketFileFailed)?;
    Ok(socket_in_use)
  }

  fn cleanup_socket(&self) {
    let _ = fs::remove_file(&self.socket_path);
    println!("Removing socket: {:?}", self.socket_path);
  }
}

impl<'ex, Tx: RequestSender + Send + Sync + Clone + 'ex> Plugin<'ex, Tx> for IpcPlugin<'ex, Tx> {
  type Error = IpcServerError;

  async fn init(request_tx: Tx, executor: Arc<Executor<'ex>>) -> Result<Self, Self::Error>
  where
    Self: Sized,
  {
    let socket_path = PathBuf::from(hsm_ipc::socket_path());
    if Self::is_socket_in_use(&socket_path)? {
      return Err(IpcServerError::SocketInUse);
    }

    Ok(Self {
      socket_path,
      request_tx,
      executor,
    })
  }

  async fn on_event(&self, _event: hsm_ipc::Event) -> Result<(), Self::Error> {
    Ok(())
  }

  async fn run(&self) -> Result<(), Self::Error> {
    let listener =
      UnixListener::bind(&self.socket_path).map_err(IpcServerError::FailedToCreateSocket)?;

    while let Some(stream) = listener.incoming().next().await {
      let request_tx = self.request_tx.clone();

      self
        .executor
        .spawn(async {
          let res = if let Ok(stream) = stream {
            StreamHandler::new(request_tx).handle_stream(stream).await
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

impl<'ex, Tx> Drop for IpcPlugin<'ex, Tx> {
  fn drop(&mut self) {
    self.cleanup_socket();
  }
}

struct StreamHandler<Tx> {
  request_tx: Tx,
}

impl<Tx> StreamHandler<Tx> {
  fn new(request_tx: Tx) -> Self {
    Self { request_tx }
  }
}

impl<Tx: RequestSender> StreamHandler<Tx> {
  async fn handle_stream(&self, stream: UnixStream) -> io::Result<()> {
    let mut request_data = String::new();
    let mut stream_reader = BufReader::new(stream);
    stream_reader.read_line(&mut request_data).await?;

    let reply_data = self.request_tx.send_json(request_data).await;

    let mut stream = stream_reader.into_inner();
    stream.write_all(&reply_data.as_bytes()).await?;

    Ok(())
  }
}
