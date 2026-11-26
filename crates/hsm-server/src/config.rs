pub mod database;
pub mod player;

use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use directories::ProjectDirs;
use kdl_config::{KdlConfigError, KdlNode, ParseConfig};
use log::{error, info};

use crate::config::{database::DatabaseConfig, player::PlayerConfig};

pub const DEFAULT_CONFIG_KDL: &str = include_str!("../default-config.kdl");

pub static PARSE_CONFIG: LazyLock<ParseConfig> = LazyLock::new(|| ParseConfig {
    default_placement: kdl_config::DefaultPlacement::Exhaustive,
    default_bool: kdl_config::BoolMode::PresenceAndValue,
    default_flag_style: kdl_config::FlagStyle::Both,
    default_conflict: kdl_config::ConflictPolicy::Error,
    deny_unknown: true,
    expand_fragments: false,
    fragment_syntax: kdl_config::FragmentSyntax::Sigil,
    root_mode: kdl_config::RootMode::Document {
        name: "config".into(),
    },
});

fn parse_config_sync(path: &Path) -> Result<HsmServerConfig, Box<KdlConfigError>> {
    if !fs::exists(path).map_err(|err| {
        Box::new(KdlConfigError::custom(
            "KDL file",
            format!("failed to read {}: {err}", path.display()),
        ))
    })? {
        info!("Creating default config file at {}", path.display());

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                Box::new(KdlConfigError::custom(
                    "KDL file",
                    format!("failed to write {}: {err}", path.display()),
                ))
            })?;
        }

        fs::write(path, DEFAULT_CONFIG_KDL).map_err(|err| {
            Box::new(KdlConfigError::custom(
                "KDL file",
                format!("failed to write {}: {err}", path.display()),
            ))
        })?;
    }

    let config = kdl_config::parse_file_with_config(path, &PARSE_CONFIG).map_err(Box::new)?;
    info!("Loaded config file from {}", path.display());

    Ok(config)
}

pub async fn parse_config_file(path: PathBuf) -> Result<HsmServerConfig, Box<KdlConfigError>> {
    smol::unblock(move || parse_config_sync(&path)).await
}

pub fn config_file_path() -> Result<PathBuf, Box<KdlConfigError>> {
    if let Some(env_path) = env::var_os("HSM_SERVER_CONFIG") {
        return Ok(PathBuf::from(env_path));
    }

    ProjectDirs::from("", "", "homeslashmusic")
        .map(|dirs| dirs.config_dir().join("config.kdl"))
        .ok_or_else(|| {
            error!("Could not determine a config path from environment or home directory");
            Box::new(KdlConfigError::custom(
                "KDL file",
                "No config path provided",
            ))
        })
}

#[derive(Debug, Clone, PartialEq, KdlNode)]
#[kdl(node = "config", rename_all = "kebab-case")]
pub struct HsmServerConfig {
    #[kdl(child, default)]
    pub database: DatabaseConfig,

    #[kdl(child, default)]
    pub player: PlayerConfig,
}

#[cfg(test)]
mod test {
    use crate::config::{DEFAULT_CONFIG_KDL, PARSE_CONFIG};

    #[test]
    fn default_config_parses() {
        kdl_config::parse_config_with_config(DEFAULT_CONFIG_KDL, &PARSE_CONFIG)
            .expect("bundled default-config.kdl must always be valid");
    }
}
