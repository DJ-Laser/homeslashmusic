use std::sync::Arc;

use hsm_ipc::{InsertItem, QueuedTrackMetadata};
use slotmap::Key as _;
use thiserror::Error;

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

#[derive(Debug, Error)]
pub enum InsertItemError {
    #[error(transparent)]
    Missing(#[from] MissingDatabaseId),
    #[error("Song {0} has no tracks to load")]
    SongNoTracks(SongId),
    #[error("Album {0} has no track groups to load")]
    AlbumNoTrackGroups(AlbumId),
}

impl DatabaseStore {
    fn queue_metadata(
        &self,
        track_id: TrackId,
    ) -> Result<(TrackLocation, Arc<QueuedTrackMetadata>), MissingDatabaseId> {
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

        let artist_text = song.map_or_else(
            || self.get_artists_text(track.artists_text.as_ref(), &track.artists),
            |song| self.get_artists_text(song.artists_text.as_ref(), &song.artists),
        )?;

        Ok((
            track.location.clone(),
            Arc::new(QueuedTrackMetadata {
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
            }),
        ))
    }

    pub fn queue_single_metadata(
        &self,
        track_id: TrackId,
    ) -> Result<Vec<(TrackLocation, Arc<QueuedTrackMetadata>)>, MissingDatabaseId> {
        Ok(vec![self.queue_metadata(track_id)?])
    }

    pub fn queue_many_metadata(
        &self,
        track_ids: &[TrackId],
    ) -> Result<Vec<(TrackLocation, Arc<QueuedTrackMetadata>)>, MissingDatabaseId> {
        track_ids
            .iter()
            .map(|track_id| self.queue_metadata(*track_id))
            .collect()
    }

    pub fn queue_insertion(
        &self,
        item: &InsertItem,
    ) -> Result<Vec<(TrackLocation, Arc<QueuedTrackMetadata>)>, InsertItemError> {
        let tracks = match *item {
            InsertItem::Track(track_id) => self.queue_single_metadata(track_id.into())?,
            InsertItem::Song(song_id) => {
                let song_id = song_id.into();
                let song = self.get_song(song_id)?;

                self.queue_single_metadata(
                    *song
                        .derived
                        .tracks()
                        .first()
                        .ok_or(InsertItemError::SongNoTracks(song_id))?,
                )?
            }

            InsertItem::TrackGroup(track_group_id) => {
                let track_group_id = track_group_id.into();
                let track_group = self.get_track_group(track_group_id)?;

                self.queue_many_metadata(track_group.derived.tracks())?
            }
            InsertItem::Album(album_id) => {
                let album_id = album_id.into();
                let album = self.get_album(album_id)?;
                let track_group_id = *album
                    .derived
                    .track_groups()
                    .first()
                    .ok_or(InsertItemError::AlbumNoTrackGroups(album_id))?;

                let track_group = self.get_track_group(track_group_id)?;

                self.queue_many_metadata(track_group.derived.tracks())?
            }
        };

        Ok(tracks)
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
