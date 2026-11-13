use serde::{Deserialize, Serialize};

/// The location of a resource (audio or metadata) provided by a plugin
/// Includes the plugin that has it, and a plugin-specific id to identify the track
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceLocation {
    pub plugin: String,
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MusicBrainzId(uuid::Uuid);

impl MusicBrainzId {
    pub fn parse_str(uuid_str: &str) -> Result<Self, uuid::Error> {
        Ok(Self(uuid::Uuid::try_parse(uuid_str)?))
    }
}

impl TryFrom<&str> for MusicBrainzId {
    type Error = uuid::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse_str(value)
    }
}
