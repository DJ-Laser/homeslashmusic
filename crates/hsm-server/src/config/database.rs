use std::{collections::HashMap, path::PathBuf, time::Duration};

use directories::ProjectDirs;
use kdl_config::{KdlChoice, KdlNode};
use log::warn;

#[derive(Debug, Clone, PartialEq, Eq, KdlNode)]
#[kdl(node = "local", rename_all = "kebab-case")]
pub struct LocalSourceConfig {
    #[kdl(positional = 0)]
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, KdlChoice)]
#[kdl(rename_all = "kebab-case")]
pub enum SourceConfig {
    Local(LocalSourceConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, KdlNode)]
#[kdl(rename_all = "kebab-case")]
pub struct ProviderConfig {
    #[kdl(children)]
    pub sources: Vec<SourceConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, KdlNode)]
#[kdl(node = "database", rename_all = "kebab-case")]
pub struct DatabaseConfig {
    #[kdl(value, name = "path", default_fn = "default_database_path")]
    pub database_file_path: PathBuf,

    #[kdl(value, from = "kdl_config::Duration")]
    pub autosave_debounce: Duration,

    #[kdl(value, from = "kdl_config::Duration")]
    pub autosave_max_delay: Duration,

    #[kdl(path = "providers", registry, default)]
    pub providers: HashMap<String, ProviderConfig>,
}

fn default_database_path() -> PathBuf {
    ProjectDirs::from("", "", "homeslashmusic").map_or_else(
        || {
            warn!(
                "Home directory could not be determined. Placing database at ./homeslashmusic.db"
            );
            PathBuf::from("./homeslashmusic.db")
        },
        |dirs| dirs.data_local_dir().join("homeslashmusic.db"),
    )
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_file_path: default_database_path(),
            autosave_debounce: Duration::from_secs(5),
            autosave_max_delay: Duration::from_secs(30),
            providers: HashMap::default(),
        }
    }
}
