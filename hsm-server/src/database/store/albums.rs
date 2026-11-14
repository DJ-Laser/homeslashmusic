use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;

use crate::database::store::{
    ArtistData, ArtistId, DatabaseStore, MissingDatabaseId, MusicBrainzId, ResourceLocation,
    TrackId,
};

slotmap::new_key_type! {
    pub struct AlbumId;
}

#[derive(Debug, Clone, Default)]
pub struct DerivedAlbumData {
    pub(super) tracks: Vec<TrackId>,
}

impl DerivedAlbumData {
    /// Tracks in this album
    pub fn songs(&self) -> &Vec<TrackId> {
        &self.tracks
    }

    pub(super) fn clear(&mut self) {
        self.tracks.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumData {
    /// The `MusicBrainzId` for this album
    pub mbid: Option<MusicBrainzId>,
    /// Album data derrived other database entries
    #[serde(skip)]
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
        self.album_by_mbid.get(mbid).copied()
    }

    pub fn insert_album(&mut self, album: AlbumData) -> AlbumId {
        self.albums.insert_with_key(|id| {
            if let Some(mbid) = album.mbid {
                self.album_by_mbid.insert(mbid, id);
            }

            Self::update_album_links(&mut self.artists, id, &album, false);

            album
        })
    }

    pub(super) fn update_album_links(
        artists: &mut DenseSlotMap<ArtistId, ArtistData>,
        id: AlbumId,
        album: &AlbumData,
        remove: bool,
    ) {
        for artist_id in &album.artists {
            let Some(artist) = artists.get_mut(*artist_id) else {
                continue;
            };

            if remove {
                artist.derived.albums.retain(|stored_id| *stored_id != id);
            } else if !artist.derived.albums.contains(&id) {
                artist.derived.albums.push(id);
            }
        }
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

        Self::update_album_links(&mut self.artists, id, db_album, true);
        Self::update_album_links(&mut self.artists, id, &album, false);

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
