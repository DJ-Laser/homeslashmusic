use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::database::store::{
    AlbumId, ArtistId, DatabaseStore, MissingDatabaseId, MusicBrainzId, ResourceLocation,
};

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
    pub name: Option<String>,
    /// Cover art for the track
    pub image: Option<ResourceLocation>,
    /// Artists associated with this track, sorted as they appear on the track metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the track's artists. May include separators like "feat."
    pub artists_text: Option<String>,
    /// The track's album, if it is a part of an album
    pub album: Option<AlbumId>,
    /// The track's track numver
    pub track_number: Option<u32>,
    /// The genres associated with this track, sorted by relevance
    pub genres: Vec<String>,
    /// The date this track was released
    pub date: Option<String>,
    /// The length of this track
    pub length: Option<Duration>,
}

impl DatabaseStore {
    pub fn get_track(&self, id: TrackId) -> Result<&TrackData, MissingDatabaseId> {
        self.tracks.get(id).ok_or(MissingDatabaseId::Track(id))
    }

    pub fn lookup_track_by_mbid(&self, mbid: &MusicBrainzId) -> Option<TrackId> {
        self.lookup.track_by_mbid.get(mbid).copied()
    }

    pub fn insert_track(&mut self, track: TrackData) -> TrackId {
        self.tracks.insert_with_key(|id| {
            self.lookup.update_track(id, &track);
            track
        })
    }
}
