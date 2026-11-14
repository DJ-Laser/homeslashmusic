use std::{collections::HashMap, fmt::Display, mem::DropGuard};

use serde::{Deserialize, Serialize};
use smol::channel::Receiver;

use crate::database::{
    jobs::JobContext,
    sources::musicbrainz::{fetch_artist, fetch_recording, fetch_release_group},
    store::DatabaseStore,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MbidQueryType {
    Recording,
    ReleaseGroup,
    Artist,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MbidQueryState {
    pub(super) incomplete: HashMap<MusicBrainzId, MbidQueryType>,
    #[serde(skip)]
    pub(super) pending: HashMap<MusicBrainzId, MbidQueryType>,
    pub(super) failed: HashMap<MusicBrainzId, MbidQueryType>,
}

impl MbidQueryState {
    /// Add a `MusicBrainzId` to the incomplete set
    /// Does nothing if a query is already pending
    pub fn insert_incomplete(&mut self, mbid: MusicBrainzId, query_type: MbidQueryType) {
        if !self.pending.contains_key(&mbid) {
            self.incomplete.insert(mbid, query_type);
            self.failed.remove(&mbid);
        }
    }

    /// Get an incomplete mbid and mark it as pending
    fn claim_next_incomplete(&mut self) -> Option<(MusicBrainzId, MbidQueryType)> {
        self.incomplete
            .iter()
            .filter(|(mbid, _)| !self.pending.contains_key(mbid))
            .map(|(&mbid, &query_type)| (mbid, query_type))
            .next()
    }

    /// Return a pending mbid to the incomplete set
    /// Use this instead of fail if the operation was aborted, but did not fail
    fn unclaim(&mut self, mbid: MusicBrainzId) {
        if let Some(query_type) = self.pending.remove(&mbid) {
            self.incomplete.insert(mbid, query_type);
        }
    }

    /// Mark the query for the specified mbid as completed and remove it from the incomplete/pending/fail sets
    fn complete(&mut self, mbid: MusicBrainzId) {
        self.incomplete.remove(&mbid);
        self.pending.remove(&mbid);
        self.failed.remove(&mbid);
    }

    /// Mark the query for the specified mbid as failed and remove it from the pending set
    /// If the mbid was not in the pending set, does nothing
    fn fail(&mut self, mbid: MusicBrainzId) {
        if let Some(query_type) = self.pending.remove(&mbid) {
            self.failed.insert(mbid, query_type);
        }
    }
}

impl DatabaseStore {
    pub fn set_mbid_incomplete(&mut self, mbid: MusicBrainzId, query_type: MbidQueryType) {
        self.mbid_query_state.insert_incomplete(mbid, query_type);
    }

    pub fn complete_mbid_query(&mut self, mbid: MusicBrainzId) {
        self.mbid_query_state.complete(mbid);
    }

    pub fn has_incomplete_mbids(&self) -> bool {
        !self.mbid_query_state.incomplete.is_empty()
    }
}

pub async fn fetch_incomplete_mbids(ctx: JobContext, waker: Receiver<()>) {
    loop {
        let Some((mbid, query_type)) =
            ctx.write_db(|db| db.mbid_query_state.claim_next_incomplete())
        else {
            if waker.recv().await.is_err() {
                break;
            }

            continue;
        };

        let guard = DropGuard::new((), |()| {
            ctx.write_db(|db| db.mbid_query_state.unclaim(mbid));
        });

        let result = match query_type {
            MbidQueryType::Recording => fetch_recording(&ctx, mbid).await,
            MbidQueryType::ReleaseGroup => fetch_release_group(&ctx, mbid).await,
            MbidQueryType::Artist => fetch_artist(&ctx, mbid).await,
        };

        match result {
            Ok(()) => ctx.write_db(|db| db.complete_mbid_query(mbid)),
            Err(err) => {
                ctx.write_db(|db| db.mbid_query_state.fail(mbid));
                eprintln!("Failed to fetch musicbrainz: {err}");
            }
        }

        DropGuard::dismiss(guard);
    }
}
