pub mod local_files;
pub mod musicbrainz;

use std::path::PathBuf;

use log::info;
use serde::{Deserialize, Serialize};

use crate::{
    config::database::{LocalSourceConfig, ProviderConfig, SourceConfig},
    database::jobs::JobContext,
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

pub async fn update_source(ctx: JobContext, source: SourceConfig) {
    match source {
        SourceConfig::Local(LocalSourceConfig { path }) => {
            info!("Loading local files from {}", path.display());
            local_files::load_all_from_path(ctx.clone(), path).await;
        }
    }
}

pub async fn update_sources(ctx: JobContext) {
    info!("Starting database update");

    let providers = ctx.read_config(|config| config.providers.clone());

    for (name, ProviderConfig { sources }) in providers {
        info!("Updating sources for provider {name}");
        for source in sources {
            update_source(ctx.clone(), source).await;
        }
    }

    info!("Finished databse update");
}
