use std::{collections::BTreeMap, time::Duration};

pub type PluginId = String;
pub type ImageId = String;
pub type TrackId = String;
pub type AlbumId = String;
pub type ArtistId = String;
pub type PlaylistId = String;
pub type MixId = String;

/// The location a track is available to listen from
/// Includes the plugin that has it, and a plugin-specific id to identify the track
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackLocation {
    plugin: PluginId,
    id: String,
}

/// The location a resource's metadata is from
/// Includes the plugin that has it, and a plugin-specific id to identify the resource
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataLocation {
    plugin: PluginId,
    id: String,
}

#[derive(Debug, Clone)]
pub struct TrackData {
    locations: Vec<TrackLocation>,
    metadata_location: MetadataLocation,

    /// Name of the track
    name: String,
    /// Cover art for the track
    image: Option<ImageId>,
    /// Artists associated with this track, sorted as they appear on the track metadata
    artists: Vec<ArtistId>,
    /// The track's album and track number, if it is a part of an album
    album: Option<(AlbumId, u64)>,
}

#[derive(Debug, Clone)]
pub struct AlbumData {
    metadata_location: MetadataLocation,
    /// Tracks in this album
    tracks: Vec<TrackId>,

    /// Name of the album
    name: String,
    /// Cover art for the album
    image: Option<ImageId>,
    /// Artists assocated with this album, sorted as they appear on the album metadata
    artists: Vec<ArtistId>,
}

#[derive(Debug, Clone)]
pub struct ArtistData {
    metadata_location: MetadataLocation,
    /// Albums by this artist, unsorted
    albums: Vec<AlbumId>,
    /// Tracks (not in an album) by this artist, unsorted
    tracks: Vec<TrackId>,

    /// Name of the artist
    name: String,
    /// Profile image for the artist
    image: Option<ImageId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaylistEntry {
    Track(TrackId),
    Album(AlbumId),
    Playlist(PlaylistId),
    Mix(MixId),
}

#[derive(Debug, Clone)]
pub struct PlaylistData {
    /// All entries in the playlist, sorted by play order
    entries: Vec<PlaylistEntry>,

    /// Name of the playlist
    name: String,
    /// Cover image for the playlist
    image: Option<ImageId>,
}

/// Map of track associations and timestamps, keyed by start time.
/// Track associations continue until the next key or end of mix
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixTracks(BTreeMap<Duration, Option<TrackId>>);

impl MixTracks {
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

#[derive(Debug, Clone)]
pub struct MixData {
    tracks: MixTracks,

    /// Name of the mix
    name: String,
    /// Cover image for the mix
    image: Option<ImageId>,
}
