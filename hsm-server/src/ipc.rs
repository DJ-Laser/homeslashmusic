use std::{convert::Infallible, path::Path};

use hsm_ipc::{
  Reply, requests, responses,
  server::{RequestHandler, handle_request},
};
use smol::{
  fs,
  io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::unix::{UnixListener, UnixStream},
  stream::StreamExt,
};

pub async fn run_ipc_server() -> Result<Infallible, io::Error> {
  let socket_path = Path::new(hsm_ipc::socket_path());

  if socket_path.exists() {
    fs::remove_file(&socket_path).await.unwrap();
  }

  let listener = UnixListener::bind(socket_path)?;

  while let Some(stream) = listener.incoming().next().await {
    let res = if let Ok(stream) = stream {
      StreamHandler.handle_stream(stream).await
    } else {
      stream.map(|_| ())
    };

    if let Err(error) = res {
      eprintln!("failed to connect to ipc client: {}", error);
    }
  }

  unreachable!("Iterating over `Incoming` should never return `None`")
}

struct StreamHandler;

impl StreamHandler {
  async fn handle_stream(&mut self, stream: UnixStream) -> io::Result<()> {
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
  fn handle_version(&mut self, _: requests::Version) -> Reply<requests::Version> {
    Ok(responses::Version(hsm_ipc::version()))
  }
}
