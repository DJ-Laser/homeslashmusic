use serde::{Deserialize, Serialize};

use crate::database::store::{AlbumId, MusicBrainzId, ResourceLocation, TrackId};

slotmap::new_key_type! {
    pub struct ArtistId;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistData {
    /// The `MusicBrainzId` for this artist
    pub mbid: Option<MusicBrainzId>,
    /// Albums by this artist, unsorted
    pub albums: Vec<AlbumId>,
    /// Tracks (not in an album) by this artist, unsorted
    pub tracks: Vec<TrackId>,

    /// Name of the artist
    pub name: String,
    /// Profile image for the artist
    pub image: Option<ResourceLocation>,
    /// The genres associated with this artist, sorted by relevance
    pub genres: Vec<String>,
}
