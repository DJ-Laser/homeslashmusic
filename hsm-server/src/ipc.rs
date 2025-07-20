use std::{fs, path::PathBuf, sync::Arc};

use hsm_ipc::{
  Reply, requests, responses,
  server::{RequestHandler, handle_request},
};
use smol::{
  Executor,
  io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::unix::{UnixListener, UnixStream},
  stream::StreamExt,
};

pub struct IpcServer<'ex> {
  socket_path: PathBuf,
  ex: Arc<Executor<'ex>>,
}

impl<'ex> IpcServer<'ex> {
  pub fn new(ex: Arc<Executor<'ex>>) -> Self {
    Self {
      socket_path: PathBuf::from(hsm_ipc::socket_path()),
      ex,
    }
  }

  pub async fn run(&mut self) -> Result<(), io::Error>
where {
    let listener = UnixListener::bind(&self.socket_path)?;

    while let Some(stream) = listener.incoming().next().await {
      self
        .ex
        .spawn(async move {
          let res = if let Ok(stream) = stream {
            StreamHandler::new().handle_stream(stream).await
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

struct StreamHandler {}

impl StreamHandler {
  fn new() -> Self {
    Self {}
  }

  async fn handle_stream(&self, stream: UnixStream) -> io::Result<()> {
    let mut request_data = String::new();
    let mut stream_reader = BufReader::new(stream);
    stream_reader.read_line(&mut request_data).await?;

    let reply_data = handle_request(&request_data, self);

    let mut stream = stream_reader.into_inner();
    stream.write_all(&reply_data.as_bytes()).await?;

    Ok(())
  }
}

impl RequestHandler for StreamHandler {
  fn handle_version(&self, _: requests::Version) -> Reply<requests::Version> {
    Ok(responses::Version(hsm_ipc::version()))
  }
}
