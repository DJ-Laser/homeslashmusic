use crate::{
    Error, MAX_PARALLEL_REQUEST_LEN, MAX_REQUEST_BYTES, MAX_SEQUENTIAL_REQUEST_LEN,
    api::{RequestMessage, ResponseMessage},
};

use super::{Request, requests};
use requests::private::_handle_single;
pub use requests::private::RequestHandler;

/// Handle a request from a hsm client
pub async fn handle_request<R: RequestHandler>(request_data: &str, request_handler: &R) -> String {
    let response = handle_request_internal(request_data, request_handler).await;
    let mut reply_data =
        serde_json::to_string(&response).expect("Responses should not fail to serialize");
    reply_data.push('\n');
    reply_data
}

async fn handle_request_internal<R: RequestHandler>(
    request_data: &str,
    request_handler: &R,
) -> ResponseMessage {
    if request_data.len() > MAX_REQUEST_BYTES {
        return vec![Err(Error::BadFormat(format!(
            "Max length of message payload exceeded (max {MAX_REQUEST_BYTES} bytes)"
        )))];
    }

    let request = match serde_json::from_str::<RequestMessage>(request_data) {
        Ok(request) => request,
        Err(error) => {
            return vec![Err(Error::BadFormat(error.to_string()))];
        }
    };

    match request {
        RequestMessage::Single(request) => vec![_handle_single(request, request_handler).await],
        RequestMessage::Sequential(requests) => {
            if requests.len() > MAX_SEQUENTIAL_REQUEST_LEN {
                return vec![Err(Error::BadFormat(format!(
                    "Max length for sequential request exceeded (max {MAX_SEQUENTIAL_REQUEST_LEN} requests)"
                )))];
            }

            let mut responses = Vec::with_capacity(requests.len());

            for request in requests {
                let response = _handle_single(request, request_handler).await;
                let is_error = response.is_err();
                responses.push(response);
                if is_error {
                    break;
                }
            }

            responses
        }
        RequestMessage::Parallel(requests) => {
            if requests.len() > MAX_PARALLEL_REQUEST_LEN {
                return vec![Err(Error::BadFormat(format!(
                    "Max length for parallel request exceeded: (max {MAX_PARALLEL_REQUEST_LEN} requests)"
                )))];
            }

            request_handler
                .handle_parallel(
                    requests
                        .into_iter()
                        .map(|request| _handle_single(request, request_handler))
                        .collect(),
                )
                .await
        }
    }
}

pub(crate) fn serialize_response<R: Request>(response: R::Response) -> String {
    serde_json::to_string(&Ok::<R::Response, Error>(response))
        .expect("Responses should not fail to serialize")
}
