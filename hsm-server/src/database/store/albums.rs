use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::database::store::{
    ArtistId, DatabaseStore, MissingDatabaseId, MusicBrainzId, ResourceLocation, SongId,
};

slotmap::new_key_type! {
    pub struct AlbumId;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DerivedAlbumData {
    songs: BTreeMap<u32, SongId>,
}

impl DerivedAlbumData {
    /// songs and song numbers in this album
    pub fn songs(&self) -> &BTreeMap<u32, SongId> {
        &self.songs
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlbumData {
    /// The `MusicBrainzId` for this album
    pub mbid: Option<MusicBrainzId>,
    /// Album data derrived other database entries
    pub derived: DerivedAlbumData,

    /// Name of the album
    pub title: Option<String>,
    /// Cover art for the album
    pub image: Option<ResourceLocation>,
    /// The date this album was released
    pub date: Option<String>,
    /// The genres associated with this album, sorted by relevance
    pub genres: Vec<String>,
    /// The total number of tracks included in this album
    pub track_count: Option<u32>,

    /// Artists assocated with this album, sorted as they appear on the album metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the album's artists. May include separators like "feat."
    pub artists_text: Option<String>,
}

impl DatabaseStore {
    pub fn get_album(&self, id: AlbumId) -> Result<&AlbumData, MissingDatabaseId> {
        self.albums.get(id).ok_or(MissingDatabaseId::Album(id))
    }

    pub fn lookup_album_by_mbid(&self, mbid: &MusicBrainzId) -> Option<AlbumId> {
        self.lookup.album_by_mbid.get(mbid).copied()
    }

    pub fn insert_album(&mut self, album: AlbumData) -> AlbumId {
        self.albums.insert_with_key(|id| {
            self.lookup.update_album(id, &album);
            album
        })
    }

    pub fn update_or_insert_album_by_mbid(
        &mut self,
        mbid: MusicBrainzId,
        mut album: AlbumData,
    ) -> Result<AlbumId, MissingDatabaseId> {
        let Some(id) = self.lookup_album_by_mbid(&mbid) else {
            album.mbid = Some(mbid);
            return Ok(self.insert_album(album));
        };

        let db_album = self
            .albums
            .get_mut(id)
            .ok_or(MissingDatabaseId::Album(id))?;

        db_album.title = album.title;
        db_album.image = album.image;
        db_album.artists = album.artists;
        db_album.artists_text = album.artists_text;
        db_album.genres = album.genres;
        db_album.date = album.date;
        db_album.track_count = album.track_count;

        Ok(id)
    }
}
