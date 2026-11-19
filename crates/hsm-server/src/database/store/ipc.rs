use slotmap::Key as _;

use crate::database::store::{
    AlbumId, ArtistId, SearchableEntityId, SongId, TrackGroupId, TrackId,
};

macro_rules! key_ipc_impls {
    ($name:ident) => {
        impl From<hsm_ipc::$name> for $name {
            fn from(value: hsm_ipc::$name) -> Self {
                slotmap::KeyData::from_ffi(value.0).into()
            }
        }

        impl From<$name> for hsm_ipc::$name {
            fn from(value: $name) -> Self {
                hsm_ipc::$name(value.data().as_ffi())
            }
        }
    };
}

key_ipc_impls!(TrackId);
key_ipc_impls!(TrackGroupId);
key_ipc_impls!(SongId);
key_ipc_impls!(AlbumId);
key_ipc_impls!(ArtistId);

impl From<SearchableEntityId> for hsm_ipc::SearchResult {
    fn from(value: SearchableEntityId) -> Self {
        match value {
            SearchableEntityId::Track(track_id) => hsm_ipc::SearchResult::Track(track_id.into()),
            SearchableEntityId::TrackGroup(track_group_id) => {
                hsm_ipc::SearchResult::TrackGroup(track_group_id.into())
            }
            SearchableEntityId::Song(song_id) => hsm_ipc::SearchResult::Song(song_id.into()),
            SearchableEntityId::Album(album_id) => hsm_ipc::SearchResult::Album(album_id.into()),
            SearchableEntityId::Artist(artist_id) => {
                hsm_ipc::SearchResult::Artist(artist_id.into())
            }
        }
    }
}
