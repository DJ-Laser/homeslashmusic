use serde::{Deserialize, Serialize};

use crate::database::store::{ArtistId, MusicBrainzId, ResourceLocation, TrackId};

slotmap::new_key_type! {
    pub struct AlbumId;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlbumData {
    /// The `MusicBrainzId` for this album
    pub mbid: Option<MusicBrainzId>,
    /// Tracks in this album
    pub tracks: Vec<(u32, TrackId)>,

    /// Name of the album
    pub name: Option<String>,
    /// Cover art for the album
    pub image: Option<ResourceLocation>,
    /// Artists assocated with this album, sorted as they appear on the album metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the album's artists. May include separators like "feat."
    pub artists_text: Option<String>,
    /// The genres associated with this album, sorted by relevance
    pub genres: Vec<String>,
    /// The date this album was released
    pub date: Option<String>,
    /// The total number of tracks included in this album
    pub track_count: Option<u32>,
}
