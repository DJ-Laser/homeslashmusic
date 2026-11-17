use serde::{Deserialize, Serialize};

/// The databse identifier for a `Track`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackId(u64);

/// The database data for a `Track`
pub struct Track {}
