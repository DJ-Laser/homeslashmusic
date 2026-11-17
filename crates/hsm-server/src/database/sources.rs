pub mod local_files;
pub mod musicbrainz;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
