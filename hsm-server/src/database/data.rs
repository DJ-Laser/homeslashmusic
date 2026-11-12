use std::{collections::BTreeMap, time::Duration};

use serde::{Deserialize, Serialize};

slotmap::new_key_type! {
    pub struct TrackId;
    pub struct AlbumId;
    pub struct ArtistId;
    pub struct PlaylistId;

}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MusicBrainzId(uuid::Uuid);

/// The location of a resource (audio or metadata) provided by a plugin
/// Includes the plugin that has it, and a plugin-specific id to identify the track
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AudioLocation {
    pubplugin: String,
    pubid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackData {
    /// A list of `PluginResource` sources for this track's audio stream
    pub audio_locations: Vec<AudioLocation>,
    /// The `MusicBrainzId` for this track
    pub mbid: Option<MusicBrainzId>,
    /// Contains the tracks included in this one if it is a mix or dj set
    pub mix_tracks: Option<MixTracks>,

    /// Name of the track
    pub name: String,
    /// Cover art for the track
    pub image: Option<AudioLocation>,
    /// Artists associated with this track, sorted as they appear on the track metadata
    pub artists: Vec<ArtistId>,
    /// The track's album and track number, if it is a part of an album
    pub album: Option<(AlbumId, u64)>,
    /// The genres associated with this track, sorted by relevance
    pub genres: Vec<String>,
    /// The date this track was released
    pub date: Option<String>,
    /// The length of this track
    pub length: Duration,
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
    pub image: Option<AudioLocation>,
    /// Artists assocated with this album, sorted as they appear on the album metadata
    pub artists: Vec<ArtistId>,
    /// The genres associated with this album, sorted by relevance
    pub genres: Vec<String>,
    /// The date this album was released
    pub date: Option<String>,
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
    pub image: Option<AudioLocation>,
    /// The genres associated with this artist, sorted by relevance
    pub genres: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaylistEntry {
    Track(TrackId),
    Album(AlbumId),
    Playlist(PlaylistId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistData {
    /// All entries in the playlist, sorted by play order
    pub entries: Vec<PlaylistEntry>,

    /// Name of the playlist
    pub name: String,
    /// Cover image for the playlist
    pub image: Option<AudioLocation>,
    /// The genres associated with this playlist, sorted by relevance
    pub genres: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MixTracks {
    List(Vec<TrackId>),
    Timestamped(MixTracksTimestamped),
}

/// Map of track associations and timestamps, keyed by start time.
/// Track associations continue until the next key or end of mix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixTracksTimestamped(BTreeMap<Duration, Option<TrackId>>);

impl MixTracksTimestamped {
    /// Create a new `MixTracks` with no track associations
    pub fn new() -> Self {
        Self(BTreeMap::default())
    }

    /// Insert a track association at the specified start time.
    /// Does not automatically end the track association, use `MixTracks::insert_empty` to do that
    pub fn insert_option(&mut self, start: Duration, track_id: Option<TrackId>) {
        if let Some((_, prev_track)) = self.0.range(..start).next_back() {
            // If the previous track is the same as the current one, inserting is redundant
            if *prev_track == track_id {
                return;
            }
        } else if track_id.is_none() {
            // If there is no prior entry and we're setting None, None is already the default
            self.0.remove(&start);
            return;
        }

        // If the immediately following transition has the same track_id, merge them
        if let Some((next_start, next_track)) = self.0.range(start..).next() {
            let next_start = *next_start;
            if next_start != start && *next_track == track_id {
                self.0.remove(&next_start);
            }
        }

        self.0.insert(start, track_id);
    }

    /// Insert a track association at the specified start time.
    /// Does not automatically end the track association, use `MixTracks::insert_empty` to do that
    pub fn insert(&mut self, start: Duration, track_id: TrackId) {
        self.insert_option(start, Some(track_id));
    }

    /// Insert an empty track association at the specified start time.
    /// Empty track associations can be used to end the previous track association.
    /// Does not automatically end the track association, use `MixTracks::insert_empty` to do that
    pub fn insert_empty(&mut self, start: Duration) {
        self.insert_option(start, None);
    }

    /// Get the track association at the specified time.
    pub fn track_at(&self, position: Duration) -> Option<TrackId> {
        self.0
            .range(..=position)
            .next_back()
            .and_then(|(_, track_id)| track_id.clone())
    }
}
