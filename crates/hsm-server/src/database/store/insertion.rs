use hsm_ipc::{InsertItem, QueuedTrackMetadata};
use thiserror::Error;

use crate::{
    audio::InsertTrackData,
    database::store::{AlbumId, DatabaseStore, MissingDatabaseId, SongId, TrackId},
};

#[derive(Debug, Error)]
pub enum InsertItemError {
    #[error(transparent)]
    Missing(#[from] MissingDatabaseId),
    #[error("Song {0} has no tracks to load")]
    SongNoTracks(SongId),
    #[error("Album {0} has no track groups to load")]
    AlbumNoTrackGroups(AlbumId),
    #[error("Playlists are not yet supported")]
    PlaylistsUnsupported,
}

impl DatabaseStore {
    fn queue_metadata(&self, track_id: TrackId) -> Result<InsertTrackData, MissingDatabaseId> {
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

        Ok(InsertTrackData {
            metadata: QueuedTrackMetadata {
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
            location: track.location.clone(),
            replay_gain: track.replay_gain.clone(),
        })
    }

    pub fn queue_single_metadata(
        &self,
        track_id: TrackId,
    ) -> Result<Vec<InsertTrackData>, MissingDatabaseId> {
        Ok(vec![self.queue_metadata(track_id)?])
    }

    pub fn queue_many_metadata(
        &self,
        track_ids: &[TrackId],
    ) -> Result<Vec<InsertTrackData>, MissingDatabaseId> {
        let mut ret: Vec<_> = track_ids
            .iter()
            .map(|track_id| self.queue_metadata(*track_id))
            .collect::<Result<_, _>>()?;

        ret.sort_by_key(|track| track.metadata.track_number);

        Ok(ret)
    }

    pub fn queue_insertion(
        &self,
        item: &InsertItem,
    ) -> Result<Vec<InsertTrackData>, InsertItemError> {
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
            InsertItem::Playlist(_) => {
                return Err(InsertItemError::PlaylistsUnsupported);
            }
        };

        Ok(tracks)
    }
}
