use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod local_files;
mod musicbrainz;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackLocation {
    Local(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackGroupLocation {
    Local { title: String, artist: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtistLocation {
    Local { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaylistLocation {
    Local(PathBuf),
}
