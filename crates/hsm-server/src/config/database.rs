use std::{collections::HashMap, path::PathBuf};

use directories::ProjectDirs;
use kdl_config::{KdlChoice, KdlNode};
use log::warn;

#[derive(Debug, Clone, KdlNode)]
#[kdl(node = "local", rename_all = "kebab-case")]
pub struct LocalSourceConfig {
    #[kdl(positional = 0)]
    pub path: PathBuf,
}

#[derive(Debug, Clone, KdlChoice)]
#[kdl(rename_all = "kebab-case")]
pub enum SourceConfig {
    Local(LocalSourceConfig),
}

#[derive(Debug, Clone, KdlNode)]
#[kdl(rename_all = "kebab-case")]
pub struct ProviderConfig {
    #[kdl(children)]
    pub sources: Vec<SourceConfig>,
}

#[derive(Debug, Clone, KdlNode)]
#[kdl(node = "database", rename_all = "kebab-case")]
pub struct DatabaseConfig {
    #[kdl(value, default_fn = "default_database_path")]
    pub path: PathBuf,

    #[kdl(path = "providers", registry, default)]
    pub providers: HashMap<String, ProviderConfig>,
}

fn default_database_path() -> PathBuf {
    ProjectDirs::from("", "", "homeslashmusic").map_or_else(
        || PathBuf::from("./homeslashmusic.db"),
        |dirs| {
            warn!(
                "Home directory could not be determined. Placing database at ./homeslashmusic.db"
            );
            dirs.data_local_dir().join("homeslashmusic.db")
        },
    )
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_database_path(),
            providers: HashMap::default(),
        }
    }
}
