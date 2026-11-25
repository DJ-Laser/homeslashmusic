mod context;
mod local_files;

use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    hash::Hash,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use log::info;
use serde::{Deserialize, Serialize};
use smol::Executor;

use crate::{
    config::database::{LocalSourceConfig, ProviderConfig, SourceConfig},
    database::{
        DatabaseHandle,
        sources::context::{MarkedEntity, UpdateContext},
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderId(String);

impl Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackLocation {
    LocalFile(PathBuf),
}

impl Display for TrackLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackLocation::LocalFile(path) => write!(f, "{}", path.display()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackGroupLocation {
    LocalTags {
        provider: ProviderId,
        title: String,
        artist: String,
    },
}

impl Display for TrackGroupLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackGroupLocation::LocalTags {
                provider: _,
                title,
                artist,
            } => {
                write!(f, "Local {title} - {artist}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtistLocation {
    LocalTags { name: String },
}

impl Display for ArtistLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtistLocation::LocalTags { name } => {
                write!(f, "Local {name}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaylistLocation {
    LocalFile(PathBuf),
}

impl Display for PlaylistLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaylistLocation::LocalFile(path) => write!(f, "{}", path.display()),
        }
    }
}

#[derive(Debug, Default)]
pub struct UpdateState {
    /// The subset of providers that were updated
    /// If a full update was done, this will be None
    pub updated_providers: Option<Vec<ProviderId>>,

    pub seen_tracks: HashMap<TrackLocation, MarkedEntity>,
    pub seen_track_groups: HashMap<TrackGroupLocation, MarkedEntity>,
    pub seen_playlists: HashMap<PlaylistLocation, MarkedEntity>,

    // Failed providers shouldn't have their unseen entities purged
    // For example, if an online provider is used when there is no wifi
    pub failed_providers: HashSet<ProviderId>,
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

    let state = Arc::new(Mutex::new(UpdateState::default()));

    for (name, ProviderConfig { sources }) in providers {
        info!("Updating sources for provider {name}");
        let ctx = UpdateContext::new(
            ProviderId(name.clone()),
            executor.clone(),
            handle.clone(),
            state.clone(),
        );

        for source in sources {
            update_source(ctx.clone(), source).await;
        }
    }

    let final_state = std::mem::take(&mut *state.lock().unwrap());
    handle.write_db(|db| db.purge_post_update(&final_state));

    info!("Finished database update");
}
