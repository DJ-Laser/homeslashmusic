use std::{collections::HashMap, path::PathBuf, time::Duration};

use directories::ProjectDirs;
use kdl_config::{KdlChoice, KdlNode};
use log::warn;

fn default_autosave_debounce() -> kdl_config::Duration {
    kdl_config::Duration::from_secs(5)
}

fn default_autosave_max_delay() -> kdl_config::Duration {
    kdl_config::Duration::from_secs(30)
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

#[derive(Debug, Clone, PartialEq, KdlNode)]
#[kdl(node = "local", rename_all = "kebab-case")]
pub struct LocalSourceConfig {
    #[kdl(positional = 0)]
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, KdlChoice)]
#[kdl(rename_all = "kebab-case")]
pub enum SourceConfig {
    Local(LocalSourceConfig),
}

#[derive(Debug, Clone, PartialEq, KdlNode)]
#[kdl(rename_all = "kebab-case")]
pub struct ProviderConfig {
    #[kdl(children_any)]
    pub sources: Vec<SourceConfig>,
}

#[derive(Debug, Clone, PartialEq, KdlNode)]
#[kdl(node = "database", rename_all = "kebab-case")]
pub struct DatabaseConfig {
    #[kdl(value, name = "path", default_fn = "default_database_path")]
    pub database_file_path: PathBuf,

    #[kdl(value, default_fn = "default_autosave_debounce")]
    autosave_debounce: kdl_config::Duration,

    #[kdl(value, default_fn = "default_autosave_max_delay")]
    autosave_max_delay: kdl_config::Duration,

    #[kdl(registry, name = "provider", select(arg(0), consume), default)]
    pub providers: HashMap<String, ProviderConfig>,
}

impl DatabaseConfig {
    pub fn autosave_debounce(&self) -> Duration {
        self.autosave_debounce.as_std()
    }

    pub fn autosave_max_delay(&self) -> Duration {
        self.autosave_max_delay.as_std()
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_file_path: default_database_path(),
            autosave_debounce: default_autosave_debounce(),
            autosave_max_delay: default_autosave_max_delay(),
            providers: HashMap::default(),
        }
    }
}
