use std::env;

use hsm_ipc::requests;
use ipc::send_request;

mod ipc;

fn main() {
  let reply = send_request(requests::LoadTrack::new(
    env::args().skip(1).next().unwrap().into(),
  ))
  .unwrap();
  println!("{reply:?}");
}
