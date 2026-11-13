use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::database::store::{AlbumId, ArtistId, MusicBrainzId, ResourceLocation};

slotmap::new_key_type! {
    pub struct TrackId;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrackData {
    /// A list of `PluginResource` sources for this track's audio stream
    pub audio_locations: Vec<ResourceLocation>,
    /// The `MusicBrainzId` for this track
    pub mbid: Option<MusicBrainzId>,
    /// Contains the tracks included in this one if it is a mix or dj set
    pub mix_tracks: Vec<TrackId>,

    /// Name of the track
    pub name: String,
    /// Cover art for the track
    pub image: Option<ResourceLocation>,
    /// Artists associated with this track, sorted as they appear on the track metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the track's artists. May include separators like "feat."
    pub artists_text: Option<String>,
    /// The track's album and track number, if it is a part of an album
    pub album: Option<(AlbumId, u64)>,
    /// The genres associated with this track, sorted by relevance
    pub genres: Vec<String>,
    /// The date this track was released
    pub date: Option<String>,
    /// The length of this track
    pub length: Option<Duration>,
}

#[derive(Debug)]
pub struct TrackInputWithMbid {
    audio_location: ResourceLocation,
    metadata_location: MusicBrainzId,

    name: String,
    image: Option<ResourceLocation>,
    artists: Vec<(MusicBrainzId, String)>,
    album: Option<(MusicBrainzId, u64)>,
    genres: Vec<String>,
    date: Option<String>,
    length: Duration,
}
