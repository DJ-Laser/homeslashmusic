pub mod database;

use std::{path::Path, sync::LazyLock};

use kdl_config::{KdlConfigError, KdlNode, ParseConfig};

use crate::config::database::DatabaseConfig;

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

pub fn parse_config_file(path: &Path) -> Result<HsmServerConfig, KdlConfigError> {
    kdl_config::parse_file_with_config(path, &PARSE_CONFIG)
}

#[derive(Debug, Clone, KdlNode)]
#[kdl(node = "config", rename_all = "kebab-case")]
pub struct HsmServerConfig {
    #[kdl(default)]
    database: DatabaseConfig,
}
