use serde::{Deserialize, Serialize};

use super::{
  client::Request, client::private::SealedRequest, responses, server::private::QualifiedRequest,
};

/// The `hsm_ipc::version()` of the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version;
impl SealedRequest for Version {
  fn qualified_request(self) -> QualifiedRequest {
    QualifiedRequest::Version(self)
  }
}
impl Request for Version {
  type Response = responses::Version;
}
