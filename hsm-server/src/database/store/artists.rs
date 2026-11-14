use std::mem;

use serde::{Deserialize, Serialize};

use crate::database::{
    sources::ArtistLocation,
    store::{
        AlbumId, DatabaseStore, Lookup, MissingDatabaseId, MusicBrainzId, SongId, TrackGroupId,
        TrackId,
    },
};

slotmap::new_key_type! {
    pub struct ArtistId;
}

#[derive(Debug, Clone, Default)]
pub struct DerivedArtistData {
    pub(super) tracks: Vec<TrackId>,
    pub(super) track_groups: Vec<TrackGroupId>,
    pub(super) songs: Vec<SongId>,
    pub(super) albums: Vec<AlbumId>,
}

impl DerivedArtistData {
    /// Tracks by this artist, unsorted
    pub fn tracks(&self) -> &Vec<TrackId> {
        &self.tracks
    }

    /// Track Groups by this artist, unsorted
    pub fn track_groups(&self) -> &Vec<TrackGroupId> {
        &self.track_groups
    }

    /// Songs by this artist, unsorted
    pub fn songs(&self) -> &Vec<SongId> {
        &self.songs
    }

    /// Albums by this artist, unsorted
    pub fn albums(&self) -> &Vec<AlbumId> {
        &self.albums
    }

    pub(super) fn clear(&mut self) {
        self.songs.clear();
        self.albums.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistData {
    /// The `MusicBrainzId` for this album
    pub mbid: Option<MusicBrainzId>,
    /// The locations of this artist
    pub locations: Vec<ArtistLocation>,
    /// Artist data derrived other database entries
    #[serde(skip)]
    pub derived: DerivedArtistData,

    /// Name of the artist
    pub name: Option<String>,
    /// Sorting key for the artist
    pub sort_name: Option<String>,
    /// The genres associated with this artist, sorted by relevance
    pub genres: Vec<String>,
}

impl DatabaseStore {
    pub fn get_artist(&self, id: ArtistId) -> Result<&ArtistData, MissingDatabaseId> {
        self.artists.get(id).ok_or(MissingDatabaseId::Artist(id))
    }

    pub fn lookup_artist_by_mbid(&self, mbid: MusicBrainzId) -> Option<ArtistId> {
        self.lookup.artist_by_mbid.get(&mbid).copied()
    }

    pub fn lookup_artist_by_location(&self, location: &ArtistLocation) -> Option<ArtistId> {
        self.lookup.artist_by_location.get(location).copied()
    }

    pub fn insert_artist(&mut self, artist: ArtistData) -> ArtistId {
        self.artists.insert_with_key(|id| {
            self.lookup
                .insert_artist(artist.mbid, &artist.locations, id);

            artist
        })
    }

    pub fn add_location_to_artist_by_mbid(
        &mut self,
        mbid: MusicBrainzId,
        location: ArtistLocation,
    ) -> Result<ArtistId, ArtistLocation> {
        let Some(id) = self.lookup_artist_by_mbid(mbid) else {
            return Err(location);
        };

        if let Some(db_artist) = self.artists.get_mut(id)
            && !db_artist.locations.contains(&location)
        {
            db_artist.locations.push(location);
        }

        Ok(id)
    }

    pub fn update_or_insert_artist_by_mbid(
        &mut self,
        mbid: MusicBrainzId,
        mut artist: ArtistData,
    ) -> Result<ArtistId, MissingDatabaseId> {
        let Some(id) = self.lookup_artist_by_mbid(mbid) else {
            artist.mbid = Some(mbid);
            return Ok(self.insert_artist(artist));
        };

        let db_artist = self
            .artists
            .get_mut(id)
            .ok_or(MissingDatabaseId::Artist(id))?;

        for location in db_artist.locations.drain(..) {
            if !artist.locations.contains(&location) {
                artist.locations.push(location);
            }
        }

        *db_artist = ArtistData {
            mbid: db_artist.mbid,
            derived: mem::replace(&mut db_artist.derived, artist.derived),
            ..artist
        };

        Ok(id)
    }
}

impl Lookup {
    pub fn insert_artist(
        &mut self,
        mbid: Option<MusicBrainzId>,
        locations: &Vec<ArtistLocation>,
        id: ArtistId,
    ) {
        if let Some(mbid) = mbid {
            self.artist_by_mbid.insert(mbid, id);
        }

        for location in locations {
            self.artist_by_location.insert(location.clone(), id);
        }
    }
}
