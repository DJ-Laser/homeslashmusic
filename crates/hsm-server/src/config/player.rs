use kdl_config::{KdlNode, KdlValue};

#[derive(Debug, Default, Clone, PartialEq, Eq, KdlValue)]
#[kdl(rename_all = "kebab-case")]
pub enum ReplayGainMode {
    Track,
    Album,
    #[default]
    Auto,
}

#[derive(Debug, Clone, PartialEq, KdlNode)]
#[kdl(node = "replay-gain", rename_all = "kebab-case")]
pub struct ReplayGainConfig {
    #[kdl(positional = 0, scalar, default)]
    pub mode: ReplayGainMode,

    #[kdl(value, name = "preamp", default)]
    pub preamp_db: f32,

    #[kdl(value, name = "fallback-preamp", default)]
    pub fallback_preamp_db: f32,

    #[kdl(value, default = "#true", default = true)]
    pub limit_clipping: bool,
}

#[derive(Debug, Default, Clone, PartialEq, KdlNode)]
#[kdl(node = "player", rename_all = "kebab-case")]
pub struct PlayerConfig {
    #[kdl(child)]
    pub replay_gain: Option<ReplayGainConfig>,
}
