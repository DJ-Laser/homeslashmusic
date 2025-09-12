use super::{Request, requests};

use requests::private::_handle_request;
pub use requests::private::RequestHandler;

pub async fn handle_request<R: RequestHandler>(
  request_data: &str,
  request_handler: &R,
) -> Result<String, (String, R::Error)> {
  let request = match serde_json::from_str(request_data) {
    Ok(request) => request,
    Err(error) => {
      println!("{}", &error);
      return Ok(crate::server::serialize_error(&error));
    }
  };

  match _handle_request(request, request_handler).await {
    Ok(reply_data) => Ok(reply_data),
    Err(error) => Err((serialize_error(&error), error)),
  }
}

pub(crate) fn serialize_response<R: Request>(response: R::Response) -> String {
  let mut reply_data = serde_json::to_string(&Ok::<R::Response, String>(response))
    .expect("Replies should not fail to serialize");
  reply_data.push('\n');
  reply_data
}

fn serialize_error(error: &impl ToString) -> String {
  let mut reply_data = serde_json::to_string(&Err::<(), String>(error.to_string()))
    .expect("Replies should not fail to serialize");
  reply_data.push('\n');
  reply_data
}
