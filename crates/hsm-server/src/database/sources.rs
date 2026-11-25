pub mod local_files;

use std::{path::PathBuf, sync::Arc};

use log::info;
use serde::{Deserialize, Serialize};
use smol::Executor;

use crate::{
    config::database::{DatabaseConfig, LocalSourceConfig, ProviderConfig, SourceConfig},
    database::{DatabaseHandle, store::DatabaseStore},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackLocation {
    LocalFile(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackGroupLocation {
    LocalTags { title: String, artist: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtistLocation {
    LocalTags { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaylistLocation {
    LocalFile(PathBuf),
}

#[derive(Debug, Clone)]
pub struct UpdateContext {
    pub executor: Arc<Executor<'static>>,
    pub handle: DatabaseHandle,
    pub provider: String,
}

impl UpdateContext {
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
}

pub async fn update_source(ctx: UpdateContext, source: SourceConfig) {
    match source {
        SourceConfig::Local(LocalSourceConfig { path }) => {
            info!("Loading local files from {}", path.display());
            local_files::load_all_from_path(ctx.clone(), path).await;
        }
    }
}

pub async fn update_sources(executor: Arc<Executor<'static>>, handle: DatabaseHandle) {
    info!("Starting database update");

    let providers = handle.read_config(|config| config.providers.clone());

    for (name, ProviderConfig { sources }) in providers {
        info!("Updating sources for provider {name}");
        for source in sources {
            update_source(
                UpdateContext {
                    executor: executor.clone(),
                    handle: handle.clone(),
                    provider: name.clone(),
                },
                source,
            )
            .await;
        }
    }

    info!("Finished databse update");
}
