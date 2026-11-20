mod jobs;
mod persistence;
mod sources;
mod store;

use std::{
    io::Write as _,
    mem,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};

use directories::ProjectDirs;
use log::{error, info};
use serde::{Deserialize, Serialize};
use smol::{
    Executor, Task,
    channel::{self, Sender},
};
use thiserror::Error;

use crate::database::{
    jobs::JobContext,
    persistence::{AutosaveTask, SaveDbError, load_db, save_db, start_autosave_task},
    store::{DatabaseStore, fetch_incomplete_mbids},
};

pub use persistence::LoadDbError;
pub use sources::TrackLocation;
pub use store::run_search;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub path: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        let default_path = ProjectDirs::from("", "", "homeslashmusic").map_or_else(
            || PathBuf::from("./homeslashmusic.db"),
            |dirs| dirs.data_local_dir().join("homeslashmusic.db"),
        );

        Self { path: default_path }
    }
}

#[derive(Debug, Error)]
pub enum ReloadConfigError {
    #[error(transparent)]
    SaveDb(#[from] SaveDbError),
}

#[derive(Debug)]
struct MusicDatabase {
    config: RwLock<DatabaseConfig>,
    db: RwLock<DatabaseStore>,
    mbid_wake_tx: Sender<()>,
    autosaver: RwLock<AutosaveTask>,
}

#[derive(Debug, Clone)]
pub struct DatabaseHandle(Arc<MusicDatabase>);

impl DatabaseHandle {
    fn ctx(&self, executor: Arc<Executor<'static>>) -> JobContext {
        JobContext::new(executor, self.clone())
    }

    pub fn read_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&DatabaseStore) -> R,
    {
        f(&self.0.db.read().unwrap())
    }

    fn write_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut DatabaseStore) -> R,
    {
        let db = &mut self.0.db.write().unwrap();
        let ret = f(db);

        self.0.autosaver.read().unwrap().wake();

        if db.has_incomplete_mbids() {
            let _ = self.0.mbid_wake_tx.try_send(());
        }

        ret
    }

    pub fn run_job<F, R>(
        &self,
        executor: &Arc<Executor<'static>>,
        job: impl FnOnce(JobContext) -> F,
    ) -> Task<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        executor.spawn(job(self.ctx(executor.clone())))
    }

    pub async fn reload_config(
        &self,
        executor: &Arc<Executor<'static>>,
        new_config: DatabaseConfig,
    ) -> Result<(), ReloadConfigError> {
        let old_config = {
            let mut guard = self.0.config.write().unwrap();
            mem::replace(&mut *guard, new_config.clone())
        };

        if new_config.path != old_config.path {
            match save_db(self, new_config.path.clone()).await {
                Ok(()) => {}
                Err(error) => {
                    // Roll back path if the database fails to save
                    error!(
                        "Failed to save database to {}. Using old path",
                        new_config.path.display()
                    );
                    self.0.config.write().unwrap().path = old_config.path;
                    return Err(error.into());
                }
            }

            let new_autosaver =
                start_autosave_task(executor, Arc::downgrade(&self.0), new_config.path);

            let old_autosaver = {
                let mut guard = self.0.autosaver.write().unwrap();
                mem::replace(&mut *guard, new_autosaver)
            };

            old_autosaver.stop().await;
            let _ = smol::fs::remove_file(&old_config.path).await;
        }

        Ok(())
    }
}

fn start_database(
    executor: &Arc<Executor<'static>>,
    store: DatabaseStore,
    config: DatabaseConfig,
) -> DatabaseHandle {
    let (mbid_wake_tx, mbid_wake_rx) = channel::bounded(1);
    let handle = DatabaseHandle(Arc::new_cyclic(|weak| {
        let autosaver = start_autosave_task(executor, weak.clone(), config.path.clone());

        MusicDatabase {
            config: RwLock::new(config),
            db: RwLock::new(store),
            mbid_wake_tx,
            autosaver: RwLock::new(autosaver),
        }
    }));

    handle
        .run_job(executor, |ctx| fetch_incomplete_mbids(ctx, mbid_wake_rx))
        .detach();

    handle
}

pub async fn load_or_init_database(
    executor: &Arc<Executor<'static>>,
    config: DatabaseConfig,
) -> Result<DatabaseHandle, LoadDbError> {
    let store = load_db(config.path.clone()).await?;

    let handle = if let Some(mut store) = store {
        store.rebuild_derived();
        info!("Loaded database file from {}", config.path.display());
        start_database(executor, store, config)
    } else {
        info!("Database file not found at {}", config.path.display());
        info!("Initializing blank database at {}", config.path.display());
        start_database(executor, DatabaseStore::default(), config)
    };

    Ok(handle)
}

pub async fn start_update(ctx: JobContext, path: PathBuf) {
    println!("Starting database update");
    println!();

    print!("Loading local files...");
    let _ = std::io::stdout().flush();
    sources::local_files::load_all_from_path(ctx.clone(), path).await;
    println!("Done!");

    ctx.read_db(DatabaseStore::print_contents);

    print!("Waiting for api requests");
    let _ = std::io::stdout().flush();

    loop {
        smol::Timer::after(Duration::from_secs(1)).await;
        print!(".");
        let _ = std::io::stdout().flush();

        if ctx.read_db(|db| !db.has_incomplete_mbids()) {
            break;
        }
    }

    println!("Done!");

    ctx.read_db(DatabaseStore::print_contents);
}
