use std::sync::Arc;

use hsm_ipc::{InsertItem, QueuedTrackMetadata};
use slotmap::Key as _;

use crate::database::{
    TrackLocation,
    store::{
        AlbumId, ArtistId, DatabaseStore, MissingDatabaseId, SearchableEntityId, SongId,
        TrackGroupId, TrackId,
    },
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
    fn queue_metadata(
        &self,
        track_id: TrackId,
    ) -> Result<Arc<(TrackLocation, QueuedTrackMetadata)>, MissingDatabaseId> {
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

        let artist_text = song
            .map(|song| self.get_artists_text(&song.artists_text, &song.artists))
            .unwrap_or_else(|| self.get_artists_text(&track.artists_text, &track.artists))?;

        Ok(Arc::new((
            track.location.clone(),
            QueuedTrackMetadata {
                track_id: track_id.into(),
                length: track.length,
                title: song
                    .map(|song| song.title.clone())
                    .unwrap_or(track.title.clone()),
                album: album
                    .map(|album| album.title.clone())
                    .or(track_group.map(|track_group| track_group.title.clone())),
                track_number: track.track_number,
                artist_text,
                date: song.and_then(|song| song.date.clone()),
                genres: song.map_or_default(|song| song.genres.clone()),
            },
        )))
    }

    pub fn queue_single_metadata(
        &self,
        track_id: TrackId,
    ) -> (Vec<Arc<(TrackLocation, QueuedTrackMetadata)>>, Vec<TrackId>) {
        if let Ok(data) = self.queue_metadata(track_id) {
            return (vec![data], Vec::new());
        }

        (Vec::new(), vec![track_id])
    }

    pub fn queue_many_metadata(
        &self,
        track_ids: &[TrackId],
    ) -> (Vec<Arc<(TrackLocation, QueuedTrackMetadata)>>, Vec<TrackId>) {
        let mut track_data = Vec::with_capacity(track_ids.len());
        let mut errors = Vec::new();

        for track_id in track_ids {
            if let Ok(data) = self.queue_metadata(*track_id) {
                track_data.push(data);
            } else {
                errors.push(*track_id);
            }
        }

        (track_data, errors)
    }

    pub fn queue_insertion(
        &self,
        item: InsertItem,
    ) -> (Vec<Arc<(TrackLocation, QueuedTrackMetadata)>>, Vec<TrackId>) {
        let mut errors = Vec::new();

        let tracks = match item {
            InsertItem::Track(track_id) => self.queue_single_metadata(track_id.into()),
            InsertItem::Song(song_id) => {
                let song = self.get_song(song_id).unwrap();

                self.queue_single_metadata(song.derived.tracks().first())
            }

            InsertItem::TrackGroup(track_group_id) => todo!(),
            InsertItem::Album(album_id) => todo!(),
        };

        (tracks, error)
    }
}

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
