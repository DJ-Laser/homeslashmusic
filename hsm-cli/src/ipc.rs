use std::{
  io::{BufRead, BufReader, Write},
  net::Shutdown,
  os::unix::net::UnixStream,
};

use hsm_ipc::{
  Reply,
  client::{Request, deserialize_reply, serialize_request},
};

pub fn send_request<R: Request>(request: R) -> Result<Reply<R>, crate::Error> {
  let socket_path = hsm_ipc::socket_path();
  let mut stream =
    UnixStream::connect(socket_path).map_err(|source| crate::Error::FailedToConnectToSocket {
      path: socket_path.into(),
      source,
    })?;

  stream
    .write_all(serialize_request(request).as_bytes())
    .map_err(crate::Error::StreamReadWrite)?;

  let mut reply_data = String::new();
  let mut stream_reader = BufReader::new(stream);
  stream_reader
    .read_line(&mut reply_data)
    .map_err(crate::Error::StreamReadWrite)?;

  stream_reader
    .into_inner()
    .shutdown(Shutdown::Both)
    .map_err(crate::Error::StreamReadWrite)?;

  let reply = deserialize_reply::<R>(&reply_data).map_err(crate::Error::Deserialize)?;
  Ok(reply)
}
