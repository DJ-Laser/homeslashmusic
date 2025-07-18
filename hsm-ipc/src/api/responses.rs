use serde::{Deserialize, Serialize};

/// A request that does not need a response was handled successfully
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handled;

/// The `hsm_ipc::version()` of the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version(String);
