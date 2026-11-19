use std::{fmt::Display, time::Duration};

use serde::{Deserialize, Serialize};

use crate::InsertItem;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MusicBrainzId(uuid::Uuid);

impl MusicBrainzId {
    pub fn parse_str(uuid_str: &str) -> Result<Self, uuid::Error> {
        Ok(Self(uuid::Uuid::try_parse(uuid_str)?))
    }
}

impl Display for MusicBrainzId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<&str> for MusicBrainzId {
    type Error = uuid::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse_str(value)
    }
}

/// The databse identifier for a `Track`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TrackSource {
    LocalFile(String),
}

/// The database data for a `Track`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// The source of this track
    pub source: TrackSource,

    /// The song this track is audio for
    pub song: Option<SongId>,
    // The track group this track is in
    pub track_group: Option<TrackGroupId>,

    /// The name of this track
    pub title: Option<String>,
    /// The length of this track
    pub length: Duration,
    /// The track number of this track
    pub track_number: Option<u32>,

    /// Artists assocated with this track, sorted as they appear on the track metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the track's artists. May include separators like "feat."
    pub artists_text: Option<String>,
}

/// The databse identifier for a `TrackGroup`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackGroupId(pub u64);

/// The location describes where to find a track group's tracks
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackGroupSource {
    LocalTags,
}

/// The database data for a `TrackGroup`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackGroup {
    /// The source of this track group
    pub source: TrackGroupSource,
    /// Tracks in this track group
    pub tracks: Vec<Track>,

    /// The album this track group is for
    pub album: Option<AlbumId>,

    /// Name of the track group
    pub title: Option<String>,
    /// The total number of tracks included in this track group
    pub track_count: Option<u32>,

    /// Artists assocated with this track group, sorted as they appear on the track group metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the track group's artists. May include separators like "feat."
    pub artists_text: Option<String>,
}

/// The databse identifier for a `Song`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongId(pub u64);

/// The database data for a `Song`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    /// The `MusicBrainzId` for this song
    pub mbid: MusicBrainzId,
    /// Tracks with audio for this song
    pub tracks: Vec<TrackId>,

    /// Name of the song
    pub title: Option<String>,
    /// The length of this song
    pub length: Option<Duration>,
    /// The date this song was released
    pub date: Option<String>,

    /// Artists associated with this song, sorted as they appear on the song metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the song's artists. May include separators like "feat."
    pub artists_text: Option<String>,

    /// The genres associated with this song, sorted by relevance
    pub genres: Vec<String>,

    /// Contains the songs included in this one if it is a mix or dj set
    pub mix_songs: Vec<SongId>,
}

/// The databse identifier for an `Album`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlbumId(pub u64);

/// The database data for a `Album`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    /// The `MusicBrainzId` for this album
    pub mbid: MusicBrainzId,
    /// Track Groups with audio for this album
    pub track_groups: Vec<TrackGroupId>,

    /// Name of the album
    pub title: Option<String>,
    /// The date this album was released
    pub date: Option<String>,
    /// The genres associated with this album, sorted by relevance
    pub genres: Vec<String>,

    /// Artists assocated with this album, sorted as they appear on the album metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the album's artists. May include separators like "feat."
    pub artists_text: Option<String>,
}

/// The databse identifier for an `Artist`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtistSource {
    LocalTags,
}

/// The database data for a `Artist`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    /// The `MusicBrainzId` for this album
    pub mbid: Option<MusicBrainzId>,
    /// The locations of this artist
    pub locations: Vec<ArtistSource>,
    /// Tracks by this artist, unsorted
    pub tracks: Vec<TrackId>,
    /// Track groups by this artist, unsorted
    pub track_groups: Vec<TrackGroupId>,
    /// Songs by this artist, unsorted
    pub songs: Vec<SongId>,
    /// Albums by this artist, unsorted
    pub albums: Vec<AlbumId>,

    /// Name of the artist
    pub name: Option<String>,
    /// Sorting key for the artist
    pub sort_name: Option<String>,
    /// The genres associated with this artist, sorted by relevance
    pub genres: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchResult {
    Track(TrackId),
    Song(SongId),
    TrackGroup(TrackGroupId),
    Album(AlbumId),
    Artist(ArtistId),
}

impl TryFrom<SearchResult> for InsertItem {
    type Error = SearchResult;

    fn try_from(value: SearchResult) -> Result<Self, Self::Error> {
        match value {
            SearchResult::Track(track_id) => Ok(InsertItem::Track(track_id)),
            SearchResult::Song(song_id) => Ok(InsertItem::Song(song_id)),
            SearchResult::TrackGroup(track_group_id) => Ok(InsertItem::TrackGroup(track_group_id)),
            SearchResult::Album(album_id) => Ok(InsertItem::Album(album_id)),
            SearchResult::Artist(artist_id) => Err(SearchResult::Artist(artist_id)),
        }
    }
}
