use serde::{Deserialize, Serialize};

use super::{Request, responses, sealed::SealedRequest};

/// The `hsm_ipc::version()` of the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version;
impl SealedRequest for Version {}
impl Request for Version {
  type Response = responses::Version;
}
