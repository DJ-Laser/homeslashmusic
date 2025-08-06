use serde::{Deserialize, Serialize};

use super::Track;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracksQuery {
  track_list: Vec<Track>,
}
