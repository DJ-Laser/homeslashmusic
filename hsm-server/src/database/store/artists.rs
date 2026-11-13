use serde::{Deserialize, Serialize};

use crate::database::store::{
    AlbumId, DatabaseStore, MissingDatabaseId, MusicBrainzId, ResourceLocation, TrackId,
};

slotmap::new_key_type! {
    pub struct ArtistId;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArtistData {
    /// The `MusicBrainzId` for this artist
    pub mbid: Option<MusicBrainzId>,
    /// Albums by this artist, unsorted
    pub albums: Vec<AlbumId>,
    /// Tracks (not in an album) by this artist, unsorted
    pub tracks: Vec<TrackId>,

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
        self.lookup.artist_by_mbid.get(mbid).copied()
    }

    pub fn insert_artist(&mut self, artist: ArtistData) -> ArtistId {
        self.artists.insert_with_key(|id| {
            self.lookup.update_artist(id, &artist);
            artist
        })
    }

    pub fn update_or_insert_artist_by_mbid(
        &mut self,
        mbid: MusicBrainzId,
        mut artist: ArtistData,
    ) -> ArtistId {
    }
}
