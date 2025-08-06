use super::{Reply, Request, requests};

pub use requests::private::{RequestHandler, handle_request};

pub(crate) fn serialize_reply<R: Request>(reply: &Reply<R>) -> String {
  let mut reply_data = serde_json::to_string(reply).expect("Replies should not fail to serialize");
  reply_data.push('\n');
  reply_data
}

pub fn serialize_error(error: String) -> String {
  let mut reply_data =
    serde_json::to_string(&Err::<(), String>(error)).expect("Replies should not fail to serialize");
  reply_data.push('\n');
  reply_data
}
