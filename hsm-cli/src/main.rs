use hsm_ipc::requests;
use ipc::send_request;

mod ipc;

fn main() {
  let reply = send_request(requests::Version).unwrap();
  println!("{reply:?}");
}
