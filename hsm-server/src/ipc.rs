use std::{convert::Infallible, fs, path::PathBuf};

use hsm_ipc::{
  Reply, requests, responses,
  server::{RequestHandler, handle_request},
};
use smol::{
  io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::unix::{UnixListener, UnixStream},
  stream::StreamExt,
};

pub struct IpcServer {
  socket_path: PathBuf,
}

impl IpcServer {
  pub fn new() -> Self {
    Self {
      socket_path: PathBuf::from(hsm_ipc::socket_path()),
    }
  }

  pub async fn run(&mut self) -> Result<Infallible, io::Error> {
    let listener = UnixListener::bind(&self.socket_path)?;

    while let Some(stream) = listener.incoming().next().await {
      let res = if let Ok(stream) = stream {
        self.handle_stream(stream).await
      } else {
        stream.map(|_| ())
      };

      if let Err(error) = res {
        eprintln!("failed to connect to ipc client: {}", error);
      }
    }

    unreachable!("Iterating over `Incoming` should never return `None`")
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

impl RequestHandler for IpcServer {
  fn handle_version(&self, _: requests::Version) -> Reply<requests::Version> {
    Ok(responses::Version(hsm_ipc::version()))
  }
}

impl Drop for IpcServer {
  fn drop(&mut self) {
    fs::remove_file(&self.socket_path);
    println!("Removing socket: `{:?}`", self.socket_path)
  }
}
