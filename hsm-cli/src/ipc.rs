use std::{
  io::{self, BufRead, BufReader, Write},
  net::Shutdown,
  os::unix::net::UnixStream,
};

use hsm_ipc::{
  Reply,
  client::{Request, deserialize_reply, serialize_request},
};

pub fn send_request<R: Request>(request: R) -> io::Result<Reply<R>> {
  let mut stream = UnixStream::connect(hsm_ipc::socket_path())?;

  stream.write_all(serialize_request(request).as_bytes())?;

  let mut reply_data = String::new();
  let mut stream_reader = BufReader::new(stream);
  stream_reader.read_line(&mut reply_data).unwrap();

  stream_reader.into_inner().shutdown(Shutdown::Both).unwrap();
  println!("{}", &reply_data);
  let reply = deserialize_reply::<R>(&reply_data)?;
  Ok(reply)
}
