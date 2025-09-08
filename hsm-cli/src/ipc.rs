use std::{
  io::{BufRead, BufReader, Write},
  net::Shutdown,
  os::unix::net::UnixStream,
};

use hsm_ipc::{
  Request,
  client::{deserialize_reply, serialize_request},
};

use crate::Error;

pub fn send_request<R: Request>(request: R) -> Result<R::Response, crate::Error> {
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

  reply.map_err(|error| Error::Server(error))
}
