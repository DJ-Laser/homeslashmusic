mod musicbrainz;
mod persistence;
mod sources;
mod store;

use std::{
    mem,
    sync::{Arc, RwLock},
};

use log::{error, info, warn};
use smol::{
    Executor,
    channel::{self},
};
use thiserror::Error;

use crate::{
    config::database::DatabaseConfig,
    database::{
        persistence::{AutosaveTask, SaveDbError, load_db, save_db, start_autosave_task},
        store::{DatabaseStore, MissingDatabaseId, fetch_incomplete_mbids},
    },
};

pub use persistence::LoadDbError;
pub use sources::TrackLocation;
pub use store::ReplayGain;

#[derive(Debug, Error)]
pub enum ReloadConfigError {
    #[error(transparent)]
    SaveDb(#[from] SaveDbError),
}

#[derive(Debug)]

struct DatabaseState {
    config: DatabaseConfig,
    store: DatabaseStore,
}

#[derive(Debug)]
struct MusicDatabase {
    state: RwLock<DatabaseState>,
    mbid_wake_tx: channel::Sender<()>,
    autosave_wake_tx: channel::Sender<()>,
}

#[derive(Debug, Clone)]
pub struct DatabaseHandle(Arc<MusicDatabase>);

impl DatabaseHandle {
    fn read_config<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&DatabaseConfig) -> R,
    {
        f(&self.0.state.read().unwrap().config)
    }

    pub fn read_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&DatabaseStore) -> R,
    {
        f(&self.0.state.read().unwrap().store)
    }

    fn write_state<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut DatabaseState) -> R,
    {
        let state = &mut self.0.state.write().unwrap();
        let ret = f(state);

        let _ = self.0.autosave_wake_tx.try_send(());

        if state.store.has_incomplete_mbids() {
            let _ = self.0.mbid_wake_tx.try_send(());
        }

        ret
    }

    fn write_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut DatabaseStore) -> R,
    {
        self.write_state(|state| f(&mut state.store))
    }

    pub async fn reload_config(&self, new_config: DatabaseConfig) -> Result<(), ReloadConfigError> {
        let old_config = {
            let mut state = self.0.state.write().unwrap();
            mem::replace(&mut state.config, new_config.clone())
        };

        if new_config.database_file_path != old_config.database_file_path {
            match save_db(self, new_config.database_file_path.clone()).await {
                Ok(()) => {}
                Err(error) => {
                    // Roll back path if the database fails to save
                    error!(
                        "Failed to save database to {}: {error}. Using old path",
                        new_config.database_file_path.display()
                    );
                    self.0.state.write().unwrap().config.database_file_path =
                        old_config.database_file_path;
                    return Err(error.into());
                }
            }

            if let Err(error) = smol::fs::remove_file(&old_config.database_file_path).await {
                warn!(
                    "Failed to remove old databse file at {}: {error}",
                    old_config.database_file_path.display()
                );
            }
        }

        Ok(())
    }

    pub async fn update(&self, executor: Arc<Executor<'static>>) {
        sources::update_sources(executor, self.clone()).await;
    }

    pub async fn search(
        &self,
        search_query: String,
    ) -> Result<Vec<(store::SearchableEntityId, u32)>, MissingDatabaseId> {
        store::run_search(self, search_query).await
    }
}

fn start_database(
    executor: &Arc<Executor<'static>>,
    store: DatabaseStore,
    config: DatabaseConfig,
) -> (DatabaseHandle, AutosaveTask) {
    store.check_integrity();

    let (mbid_wake_tx, mbid_wake_rx) = channel::bounded(1);
    let (autosave_wake_tx, autosave_wake_rx) = channel::bounded(1);

    let handle = DatabaseHandle(Arc::new(MusicDatabase {
        state: RwLock::new(DatabaseState { config, store }),
        mbid_wake_tx,
        autosave_wake_tx: autosave_wake_tx.clone(),
    }));

    executor
        .spawn(fetch_incomplete_mbids(handle.clone(), mbid_wake_rx))
        .detach();

    let autosaver = start_autosave_task(
        executor,
        handle.clone(),
        (autosave_wake_tx, autosave_wake_rx),
    );

    (handle, autosaver)
}

pub async fn load_or_init_database(
    executor: &Arc<Executor<'static>>,
    config: DatabaseConfig,
) -> Result<(DatabaseHandle, AutosaveTask), LoadDbError> {
    let store = load_db(config.database_file_path.clone()).await?;

    let handle = if let Some(mut store) = store {
        store.rebuild_derived();
        info!(
            "Loaded database file from {}",
            config.database_file_path.display()
        );
        start_database(executor, store, config)
    } else {
        info!(
            "Database file not found at {}",
            config.database_file_path.display()
        );
        info!(
            "Initializing blank database at {}",
            config.database_file_path.display()
        );
        start_database(executor, DatabaseStore::default(), config)
    };

    Ok(handle)
}
