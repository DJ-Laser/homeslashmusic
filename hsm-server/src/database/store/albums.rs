use serde::{Deserialize, Serialize};

use crate::database::store::{ArtistId, MusicBrainzId, ResourceLocation, TrackId};

slotmap::new_key_type! {
    pub struct AlbumId;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumData {
    /// The `MusicBrainzId` for this album
    pub mbid: Option<MusicBrainzId>,
    /// Tracks in this album
    pub tracks: Vec<TrackId>,

    /// Name of the album
    pub name: String,
    /// Cover art for the album
    pub image: Option<ResourceLocation>,
    /// Artists assocated with this album, sorted as they appear on the album metadata
    pub artists: Vec<ArtistId>,
    /// The genres associated with this album, sorted by relevance
    pub genres: Vec<String>,
    /// The date this album was released
    pub date: Option<String>,
}
