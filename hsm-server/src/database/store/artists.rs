use serde::{Deserialize, Serialize};

use crate::database::store::{
    AlbumId, DatabaseStore, MissingDatabaseId, MusicBrainzId, ResourceLocation, SongId,
};

slotmap::new_key_type! {
    pub struct ArtistId;
}

#[derive(Debug, Clone, Default)]
pub struct DerivedArtistData {
    pub(super) songs: Vec<SongId>,
    pub(super) albums: Vec<AlbumId>,
}

impl DerivedArtistData {
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
    /// The `MusicBrainzId` for this artist
    pub mbid: Option<MusicBrainzId>,
    /// Artist data derrived other database entries
    #[serde(skip)]
    pub derived: DerivedArtistData,

    /// Name of the artist
    pub name: Option<String>,
    /// Sorting key for the artist
    pub sort_name: Option<String>,
    /// Profile image for the artist
    pub image: Option<ResourceLocation>,
    /// The genres associated with this artist, sorted by relevance
    pub genres: Vec<String>,
}

impl DatabaseStore {
    pub fn get_artist(&self, id: ArtistId) -> Result<&ArtistData, MissingDatabaseId> {
        self.artists.get(id).ok_or(MissingDatabaseId::Artist(id))
    }

    pub fn lookup_artist_by_mbid(&self, mbid: &MusicBrainzId) -> Option<ArtistId> {
        self.artist_by_mbid.get(mbid).copied()
    }

    pub fn insert_artist(&mut self, artist: ArtistData) -> ArtistId {
        self.artists.insert_with_key(|id| {
            if let Some(mbid) = artist.mbid {
                self.artist_by_mbid.insert(mbid, id);
            }

            artist
        })
    }

    pub fn update_or_insert_artist_by_mbid(
        &mut self,
        mbid: MusicBrainzId,
        mut artist: ArtistData,
    ) -> Result<ArtistId, MissingDatabaseId> {
        let Some(id) = self.lookup_artist_by_mbid(&mbid) else {
            artist.mbid = Some(mbid);
            return Ok(self.insert_artist(artist));
        };

        let db_artist = self
            .artists
            .get_mut(id)
            .ok_or(MissingDatabaseId::Artist(id))?;

        db_artist.name = artist.name;
        db_artist.sort_name = artist.sort_name;
        db_artist.image = artist.image;
        db_artist.genres = artist.genres;

        Ok(id)
    }
}
