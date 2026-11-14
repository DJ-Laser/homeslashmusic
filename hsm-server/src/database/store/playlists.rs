use serde::{Deserialize, Serialize};

use crate::database::store::{AlbumId, DatabaseStore, MissingDatabaseId, ResourceLocation, SongId};

slotmap::new_key_type! {
    pub struct PlaylistId;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaylistEntry {
    Song(SongId),
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
    pub image: Option<ResourceLocation>,
    /// The genres associated with this playlist, sorted by relevance
    pub genres: Vec<String>,
}

impl DatabaseStore {
    pub fn get_playlist(&self, id: PlaylistId) -> Result<&PlaylistData, MissingDatabaseId> {
        self.playlists
            .get(id)
            .ok_or(MissingDatabaseId::Playlist(id))
    }

    pub fn insert_playlist(&mut self, playlist: PlaylistData) -> PlaylistId {
        self.playlists.insert(playlist)
    }
}
