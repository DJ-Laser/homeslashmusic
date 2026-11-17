use hsm_ipc::QueuedTrackMetadata;
use slotmap::Key as _;

use crate::database::{
    TrackLocation,
    store::{AlbumId, ArtistId, DatabaseStore, MissingDatabaseId, SongId, TrackGroupId, TrackId},
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

impl DatabaseStore {
    pub fn queue_track(
        &self,
        track_id: TrackId,
    ) -> Result<(TrackLocation, QueuedTrackMetadata), MissingDatabaseId> {
        let track = self.get_track(track_id)?;

        let track_group = track
            .track_group
            .map(|track_group_id| self.get_track_group(track_group_id))
            .transpose()?;

        let album = track_group
            .and_then(|track_group| track_group.album)
            .map(|album_id| self.get_album(album_id))
            .transpose()?;

        let song = track
            .song
            .map(|song_id| self.get_song(song_id))
            .transpose()?;

        Ok((
            track.location.clone(),
            QueuedTrackMetadata {
                track_id: track_id.into(),
                length: track.length,
                title: song
                    .map(|song| song.title.clone())
                    .unwrap_or(track.title.clone()),
                album: album
                    .and_then(|album| album.title.clone())
                    .or(track_group.and_then(|track_group| track_group.title.clone())),
                track_number: track.track_number,
                artist_text: song
                    .map(|song| song.artists_text.clone())
                    .unwrap_or(track.artists_text.clone()),
                date: song.and_then(|song| song.date.clone()),
                genres: song.map_or_default(|song| song.genres.clone()),
            },
        ))
    }
}
