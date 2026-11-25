#![expect(dead_code)]

use std::{
    collections::{HashMap, hash_map},
    fmt::Display,
    hash::Hash,
    ops::ControlFlow,
    sync::{Arc, Mutex},
};

use log::{error, warn};
use smol::{Executor, channel};

use crate::{
    config::database::DatabaseConfig,
    database::{
        DatabaseHandle, TrackLocation,
        sources::{PlaylistLocation, ProviderId, TrackGroupLocation, UpdateState},
        store::{DatabaseStore, PlaylistId, TrackGroupId, TrackId},
    },
};

/// Allows parallel tasks to wait for an entity that was marked but not yet added to the database
#[derive(Debug, Clone)]
pub struct MarkedEntity {
    provider: ProviderId,
    done_rx: channel::Receiver<()>,
}

#[derive(Debug)]
pub struct ClaimGuard<T> {
    pub location: T,
    done_tx: channel::Sender<()>,
}

#[derive(Debug, Clone)]
pub struct UpdateContext {
    provider: ProviderId,
    executor: Arc<Executor<'static>>,
    handle: DatabaseHandle,
    state: Arc<Mutex<UpdateState>>,
}

impl UpdateContext {
    pub fn new(
        provider: ProviderId,
        executor: Arc<Executor<'static>>,
        handle: DatabaseHandle,
        state: Arc<Mutex<UpdateState>>,
    ) -> Self {
        Self {
            provider,
            executor,
            handle,
            state,
        }
    }

    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// Spawn a new task that runs in parallel to the current task.
    /// Dropping this future will cancel it.
    pub fn spawn<F>(&self, fut: F) -> impl Future<Output = F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send,
    {
        self.executor.spawn(fut)
    }

    /// Synchronously read from the database
    pub fn read_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&DatabaseStore) -> R,
    {
        self.handle.read_db(f)
    }

    /// Synchronously read from the database
    pub fn read_config<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&DatabaseConfig) -> R,
    {
        self.handle.read_config(|config| f(config))
    }

    /// Synchronously write to the database
    pub fn write_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut DatabaseStore) -> R,
    {
        self.handle.write_state(|state| f(&mut state.store))
    }

    async fn mark<K, R>(
        &self,
        location: K,
        get_map: fn(&mut UpdateState) -> &mut HashMap<K, MarkedEntity>,
        lookup: impl Fn(&K) -> Option<R>,
        entity_name: &'static str,
    ) -> ControlFlow<R, ClaimGuard<K>>
    where
        K: Clone + Eq + Hash + Display,
    {
        let mut marked_entity = {
            let mut state = self.state.lock().unwrap();
            match get_map(&mut state).entry(location.clone()) {
                hash_map::Entry::Occupied(occupied_entry) => {
                    let marked_entity = occupied_entry.get().clone();
                    if self.provider != marked_entity.provider {
                        warn!(
                            "Provider {} added {entity_name} \"{}\" which was already marked by provider {}",
                            self.provider,
                            occupied_entry.key(),
                            marked_entity.provider
                        );
                    }

                    marked_entity
                }
                hash_map::Entry::Vacant(vacant_entry) => {
                    let (done_tx, done_rx) = channel::bounded(1);
                    vacant_entry.insert(MarkedEntity {
                        provider: self.provider.clone(),
                        done_rx,
                    });

                    return ControlFlow::Continue(ClaimGuard { location, done_tx });
                }
            }
        };

        loop {
            let _ = marked_entity.done_rx.recv().await;

            if let Some(ret) = lookup(&location) {
                return ControlFlow::Break(ret);
            }

            let mut state = self.state.lock().unwrap();

            match get_map(&mut state).get(&location) {
                // If the done_rx channel isn't the same as the old one, the MarkedEntity was updated
                // Marked entity was updated by another waiting task; loop and wait for it
                Some(new_marked_entity)
                    if !new_marked_entity
                        .done_rx
                        .same_channel(&marked_entity.done_rx) =>
                {
                    marked_entity = new_marked_entity.clone();
                }

                // Marked entity was not updated; claim it
                None | Some(_) => {
                    error!(
                        "Provider {} marked {entity_name} \"{location}\" as done, but it did not appear in the database.",
                        marked_entity.provider
                    );

                    let (done_tx, done_rx) = channel::bounded(1);
                    get_map(&mut state).insert(
                        location.clone(),
                        MarkedEntity {
                            provider: self.provider.clone(),
                            done_rx,
                        },
                    );

                    return ControlFlow::Continue(ClaimGuard { location, done_tx });
                }
            }
        }
    }

    /// Mark a `TrackLocation` as provided by the context's provider.
    /// Returns a `ControlFlow` to either use the existing entity id or proceed with entity insertion.
    pub async fn mark_track(
        &self,
        location: TrackLocation,
    ) -> ControlFlow<TrackId, ClaimGuard<TrackLocation>> {
        self.mark(
            location,
            |state| &mut state.seen_tracks,
            |location| self.handle.read_db(|db| db.lookup_track(location)),
            "track",
        )
        .await
    }

    /// Mark a `TrackGroupLocation` as provided by the context's provider.
    /// Returns a `ControlFlow` to either use the existing entity id or proceed with entity insertion.
    pub async fn mark_track_group(
        &self,
        location: TrackGroupLocation,
    ) -> ControlFlow<TrackGroupId, ClaimGuard<TrackGroupLocation>> {
        self.mark(
            location,
            |state| &mut state.seen_track_groups,
            |location| self.handle.read_db(|db| db.lookup_track_group(location)),
            "track group",
        )
        .await
    }

    /// Mark a `PlaylistLocation` as provided by the context's provider.
    /// Returns a `ControlFlow` to either use the existing entity id or proceed with entity insertion.
    pub async fn mark_playlist(
        &self,
        location: PlaylistLocation,
    ) -> ControlFlow<PlaylistId, ClaimGuard<PlaylistLocation>> {
        self.mark(
            location,
            |state| &mut state.seen_playlists,
            |location| self.handle.read_db(|db| db.lookup_playlist(location)),
            "playlist",
        )
        .await
    }

    /// Set the context's provider as failed.
    /// Failure should be temporary, and sources should log a failure reason.
    /// Failed provider's entities aren't purged at the end of an update
    pub fn set_failed(&self) {
        self.state
            .lock()
            .unwrap()
            .failed_providers
            .insert(self.provider.clone());
    }
}
