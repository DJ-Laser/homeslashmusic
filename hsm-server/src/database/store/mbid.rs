use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::database::store::DatabaseStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MusicBrainzId(uuid::Uuid);

impl MusicBrainzId {
    pub fn parse_str(uuid_str: &str) -> Result<Self, uuid::Error> {
        Ok(Self(uuid::Uuid::try_parse(uuid_str)?))
    }
}

impl Display for MusicBrainzId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<&str> for MusicBrainzId {
    type Error = uuid::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse_str(value)
    }
}

impl DatabaseStore {
    /// Add a query for a `MusicBrainzId` to the pending set
    /// Returns the mbid if a query is already pending
    pub fn claim_mbid_query(&mut self, mbid: MusicBrainzId) -> Result<(), MusicBrainzId> {
        if self.unqueried_mbids.insert(mbid) {
            self.failed_mbids.remove(&mbid);

            Ok(())
        } else {
            Err(mbid)
        }
    }

    /// Mark the query for the specified mbid as completed and remove it from the pending set
    pub fn complete_mbid_query(&mut self, mbid: MusicBrainzId) {
        self.failed_mbids.remove(&mbid);
        self.unqueried_mbids.remove(&mbid);
    }

    /// Mark the query for the specified mbid as failed and remove it from the pending set
    /// If the mbid was not in the pending set, does nothing
    pub fn fail_mbid_query(&mut self, mbid: MusicBrainzId) {
        if self.unqueried_mbids.remove(&mbid) {
            self.failed_mbids.insert(mbid);
        }
    }
}
