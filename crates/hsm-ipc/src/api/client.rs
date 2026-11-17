use std::marker::PhantomData;

use thiserror::Error;

use crate::{
    Error,
    api::{RequestMessage, ResponseMessage},
    requests::private::QualifiedRequest,
};

use super::{Request, Response};

#[derive(Debug, Error)]
pub enum ParseError {
    /// Json response could not be parsed
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Json could be parsed, but server's response structure was invalid
    #[error("Invalid response format from server: {0}")]
    BadFormat(String),

    /// Client code used `BatchToken`s improperly
    #[error("Invalid response format from server: {0}")]
    BatchToken(String),
}

fn serialize_request_internal(request: &RequestMessage) -> String {
    let mut request_data = serde_json::to_string::<RequestMessage>(request)
        .expect("Requests should not fail to serialize");
    request_data.push('\n');
    request_data
}

fn deserialize_response_internal<R: Request>(
    response: Result<String, Error>,
) -> Result<Response<R>, ParseError> {
    match response {
        Ok(response_data) => {
            let parsed = serde_json::from_str::<R::Response>(&response_data)?;
            Ok(Ok(parsed))
        }
        Err(error) => Ok(Err(error)),
    }
}

pub fn serialize_request(request: impl Request) -> String {
    serialize_request_internal(&RequestMessage::Single(request.into()))
}

pub fn deserialize_reply<R: Request>(response_data: &str) -> Result<Response<R>, ParseError> {
    let mut responses = serde_json::from_str::<ResponseMessage>(response_data)?;

    if responses.len() != 1 {
        return Err(ParseError::BadFormat(format!(
            "Expected 1 response for single request, got {}",
            responses.len()
        )));
    }

    let response = responses.pop().expect("Checked responses.len == 1");
    deserialize_response_internal::<R>(response)
}

/// Typed identifier for a request type and positioning in a `BatchRequest` and `BatchResponse`
#[derive(Debug)]
pub struct BatchToken<R: Request> {
    index: usize,
    _marker: PhantomData<fn() -> R>,
}

/// Helper to build batched requests with proper response types
#[derive(Debug, Default, Clone)]
pub struct BatchRequest {
    requests: Vec<QualifiedRequest>,
}

impl BatchRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            requests: Vec::with_capacity(capacity),
        }
    }

    pub fn push<R: Request>(&mut self, request: R) -> BatchToken<R> {
        self.requests.push(request.into());
        BatchToken {
            index: self.requests.len() - 1,
            _marker: PhantomData,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Serialize as a `Sequential` batch (aborts on first error).
    pub fn serialize_sequential(self) -> (String, BatchResponseParser) {
        let expected_len = self.requests.len();

        (
            serialize_request_internal(&RequestMessage::Sequential(self.requests)),
            BatchResponseParser {
                mode: BatchMode::Sequential,
                request_len: expected_len,
            },
        )
    }

    /// Serialize as a `Parallel` batch (executes all concurrently).
    pub fn serialize_parallel(self) -> (String, BatchResponseParser) {
        let expected_len = self.requests.len();

        (
            serialize_request_internal(&RequestMessage::Parallel(self.requests)),
            BatchResponseParser {
                mode: BatchMode::Parallel,
                request_len: expected_len,
            },
        )
    }
}

#[derive(Debug, Clone)]
enum BatchMode {
    Sequential,
    Parallel,
}

#[derive(Debug, Clone)]
pub struct BatchResponseParser {
    mode: BatchMode,
    request_len: usize,
}

impl BatchResponseParser {
    /// Attempts to deserialize the batch resposne, failing on the first error.
    /// Useful for when you only care if a batch succeeded
    pub fn deserialize_ok(self, response_data: &str) -> Result<Result<(), Error>, ParseError> {
        let responses = serde_json::from_str::<ResponseMessage>(response_data)?;

        match self.mode {
            BatchMode::Sequential => Self::validate_sequential(&responses, self.request_len)?,
            BatchMode::Parallel => Self::validate_parallel(&responses, self.request_len)?,
        }

        if let Some(error) = responses.into_iter().find_map(Result::err) {
            return Ok(Err(error));
        }

        Ok(Ok(()))
    }

    pub fn deserialize(self, response_data: &str) -> Result<BatchResponse, ParseError> {
        let responses = serde_json::from_str::<ResponseMessage>(response_data)?;

        match self.mode {
            BatchMode::Sequential => Self::validate_sequential(&responses, self.request_len)?,
            BatchMode::Parallel => Self::validate_parallel(&responses, self.request_len)?,
        }

        let sequential_abort_error = if matches!(self.mode, BatchMode::Sequential) {
            responses.last().and_then(|res| res.as_ref().err().cloned())
        } else {
            None
        };

        Ok(BatchResponse {
            responses: responses.into_iter().map(Some).collect(),
            request_len: self.request_len,
            sequential_abort_error,
        })
    }

    fn validate_sequential(
        responses: &ResponseMessage,
        request_len: usize,
    ) -> Result<(), ParseError> {
        if responses.is_empty() {
            return Err(ParseError::BadFormat(
                "An empty response list was received".into(),
            ));
        }

        if responses.len() > request_len {
            return Err(ParseError::BadFormat(format!(
                "Expected at most {request_len} responses, but got {}",
                responses.len(),
            )));
        }

        // Last item must be an error if the batch was aborted
        if responses.len() < request_len && !matches!(responses.last(), Some(Err(_))) {
            return Err(ParseError::BadFormat(format!(
                "Sequential batch truncated at {}/{} responses without an error",
                responses.len(),
                request_len
            )));
        }

        Ok(())
    }

    fn validate_parallel(
        responses: &ResponseMessage,
        request_len: usize,
    ) -> Result<(), ParseError> {
        if responses.len() != request_len {
            return Err(ParseError::BadFormat(format!(
                "Expected exactly {request_len} responses, but got {}",
                responses.len(),
            )));
        }

        Ok(())
    }
}

pub struct BatchResponse {
    request_len: usize,
    responses: Vec<Option<Result<String, Error>>>,
    sequential_abort_error: Option<Error>,
}

impl BatchResponse {
    #[expect(
        clippy::needless_pass_by_value,
        reason = "batch token must be consumed to maintain invariants"
    )]
    pub fn take<R: Request>(&mut self, token: BatchToken<R>) -> Result<Response<R>, ParseError> {
        if token.index >= self.request_len {
            return Err(ParseError::BatchToken(format!(
                "Token index {} out of bounds for response of length {}. Do not mix tokens from different batches.",
                token.index, self.request_len
            )));
        }

        if let Some(slot) = self.responses.get_mut(token.index) {
            let response = slot.take().ok_or_else(|| ParseError::BatchToken(format!(
                "Slot at token index {} already taken. Do not mix tokens from different batches.",
                token.index
            )))?;

            return deserialize_response_internal::<R>(response);
        }

        // Responses list aborted before the token index
        // Parallel batches must have all expected responses, so self must be sequential
        let error = self
            .sequential_abort_error
            .as_ref()
            .expect("Sequential BatchResponse without all responses cannot be constructed with no abort error");

        Ok(Err(error.clone()))
    }
}

/// Tests by Ai because I'm lazy
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Version,
        requests::{GetPlaybackState, GetVersion, Pause, Play},
    };

    fn mock_version() -> Version {
        Version {
            major: 1,
            minor: 2,
            patch: 3,
            commit: "abc1234".into(),
        }
    }

    /// Helper to mock a server response JSON line from a `ResponseMessage`.
    #[expect(clippy::needless_pass_by_value)]
    fn mock_wire_response(items: Vec<Result<String, Error>>) -> String {
        let mut s = serde_json::to_string(&items).unwrap();
        s.push('\n');
        s
    }

    #[test]
    fn single_request_success_and_server_error() {
        // 1. Valid response payload
        let raw_ver = serde_json::to_string(&mock_version()).unwrap();
        let wire = mock_wire_response(vec![Ok(raw_ver)]);
        let res = deserialize_reply::<GetVersion>(&wire).unwrap();
        assert!(res.is_ok());

        // 2. Server-side command error
        let wire_err = mock_wire_response(vec![Err(Error::Server("not playing".into()))]);
        let res_err = deserialize_reply::<Pause>(&wire_err).unwrap();
        assert!(matches!(res_err, Err(Error::Server(msg)) if msg == "not playing"));

        // 3. Protocol violations (0 or >1 responses for a Single request)
        let wire_empty = mock_wire_response(vec![]);
        assert!(matches!(
            deserialize_reply::<Pause>(&wire_empty),
            Err(ParseError::BadFormat(_))
        ));

        let wire_too_many = mock_wire_response(vec![Ok("null".into()), Ok("null".into())]);
        assert!(matches!(
            deserialize_reply::<Pause>(&wire_too_many),
            Err(ParseError::BadFormat(_))
        ));
    }

    #[test]
    fn sequential_batch_full_success_and_out_of_order_take() {
        let mut batch = BatchRequest::new();
        let tok_pause = batch.push(Pause);
        let tok_play = batch.push(Play);
        let tok_ver = batch.push(GetVersion);

        let (_line, parser) = batch.serialize_sequential();

        let wire = mock_wire_response(vec![
            Ok(serde_json::to_string(&()).unwrap()),
            Ok(serde_json::to_string(&()).unwrap()),
            Ok(serde_json::to_string(&mock_version()).unwrap()),
        ]);

        let mut reply = parser.deserialize(&wire).unwrap();

        // Take tokens out of order
        assert!(reply.take(tok_ver).unwrap().is_ok());
        assert!(reply.take(tok_pause).unwrap().is_ok());
        assert!(reply.take(tok_play).unwrap().is_ok());
    }

    #[test]
    fn sequential_batch_early_abort_propagates_error_to_later_tokens() {
        let mut batch = BatchRequest::new();
        let tok_0 = batch.push(Pause);
        let tok_1 = batch.push(Play); // Will fail here
        let tok_2 = batch.push(GetVersion); // Never executed by server
        let tok_3 = batch.push(GetPlaybackState); // Never executed by server

        let (_line, parser) = batch.serialize_sequential();

        // Server ran tok_0 (Ok), failed on tok_1 (Err), and aborted before tok_2/tok_3
        let wire = mock_wire_response(vec![
            Ok("null".into()),
            Err(Error::Server("audio device busy".into())),
        ]);

        let mut reply = parser.deserialize(&wire).unwrap();

        // Consume tok_1 FIRST to verify `sequential_abort_error` isn't lost when slot 1 is emptied
        let res_1 = reply.take(tok_1).unwrap();
        assert!(matches!(res_1, Err(Error::Server(ref m)) if m == "audio device busy"));

        // tok_0 still succeeded
        assert!(reply.take(tok_0).unwrap().is_ok());

        // tok_2 and tok_3 both receive the abort error
        let res_2 = reply.take(tok_2).unwrap();
        let res_3 = reply.take(tok_3).unwrap();
        assert!(matches!(res_2, Err(Error::Server(ref m)) if m == "audio device busy"));
        assert!(matches!(res_3, Err(Error::Server(ref m)) if m == "audio device busy"));
    }

    #[test]
    fn sequential_batch_rejects_truncated_reply_without_trailing_error() {
        let mut batch = BatchRequest::new();
        let _tok_0 = batch.push(Pause);
        let _tok_1 = batch.push(Play);

        let (_line, parser) = batch.serialize_sequential();

        // Only 1 response returned, and it's Ok instead of Err — invalid sequential truncation!
        let wire = mock_wire_response(vec![Ok("null".into())]);
        assert!(matches!(
            parser.deserialize(&wire),
            Err(ParseError::BadFormat(_))
        ));
    }

    #[test]
    fn parallel_batch_independent_errors_and_strict_length() {
        let mut batch = BatchRequest::new();
        let tok_0 = batch.push(Pause);
        let tok_1 = batch.push(Play);
        let tok_2 = batch.push(GetVersion);

        let (_line, parser) = batch.serialize_parallel();

        // Middle command fails, but parallel continues and succeeds on tok_2
        let wire = mock_wire_response(vec![
            Ok("null".into()),
            Err(Error::Server("failed".into())),
            Ok(serde_json::to_string(&mock_version()).unwrap()),
        ]);

        let mut reply = parser.deserialize(&wire).unwrap();
        assert!(reply.take(tok_0).unwrap().is_ok());
        assert!(reply.take(tok_1).unwrap().is_err());
        assert!(reply.take(tok_2).unwrap().is_ok());

        // Parallel batch with missing response must fail validation immediately
        let mut batch2 = BatchRequest::new();
        batch2.push(Pause);
        batch2.push(Play);
        let (_line2, parser2) = batch2.serialize_parallel();

        let short_wire = mock_wire_response(vec![Err(Error::Server("failed".into()))]);
        assert!(matches!(
            parser2.deserialize(&short_wire),
            Err(ParseError::BadFormat(_))
        ));
    }

    #[test]
    fn deserialize_ok_helper_checks_all_items() {
        let mut batch = BatchRequest::new();
        batch.push(Pause);
        batch.push(Play);
        let (_line, parser) = batch.serialize_parallel();

        // All Ok
        let wire_ok = mock_wire_response(vec![Ok("null".into()), Ok("null".into())]);
        assert!(parser.clone().deserialize_ok(&wire_ok).unwrap().is_ok());

        // Second item failed
        let wire_err =
            mock_wire_response(vec![Ok("null".into()), Err(Error::Server("boom".into()))]);
        let server_err = parser.deserialize_ok(&wire_err).unwrap().unwrap_err();
        assert!(matches!(server_err, Error::Server(msg) if msg == "boom"));
    }

    #[test]
    #[expect(clippy::similar_names)]
    fn cross_batch_token_mixups_return_batch_token_error() {
        // Batch A has 1 item
        let mut batch_a = BatchRequest::new();
        let tok_a0 = batch_a.push(Pause);
        let (_line_a, parser_a) = batch_a.serialize_parallel();

        // Batch B has 2 items
        let mut batch_b = BatchRequest::new();
        let tok_b0 = batch_b.push(Pause);
        let tok_b1 = batch_b.push(Play);
        let (_line_b, parser_b) = batch_b.serialize_parallel();

        let wire_a = mock_wire_response(vec![Ok("null".into())]);
        let wire_b = mock_wire_response(vec![Ok("null".into()), Ok("null".into())]);

        let mut reply_a = parser_a.deserialize(&wire_a).unwrap();
        let mut reply_b = parser_b.deserialize(&wire_b).unwrap();

        // 1. Passing tok_b1 (index 1) into reply_a (len 1) -> out-of-bounds BatchToken error
        assert!(matches!(
            reply_a.take(tok_b1),
            Err(ParseError::BatchToken(_))
        ));

        // 2. Taking tok_a0 from reply_b consumes slot 0, so taking tok_b0 afterward fails cleanly
        assert!(reply_b.take(tok_a0).is_ok());
        assert!(matches!(
            reply_b.take(tok_b0),
            Err(ParseError::BatchToken(_))
        ));
    }
}
