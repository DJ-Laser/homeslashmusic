use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::database::store::{AlbumId, DatabaseStore, MissingDatabaseId, SongId};

slotmap::new_key_type! {
    pub struct TrackId;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DerivedTrackData {
    albums: Vec<AlbumId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrackData {
    /// Track data derrived other database entries
    pub derived: DerivedTrackData,

    /// The song this track is audio for
    pub song: Option<SongId>,
    // The album this track is in
    pub album: Option<AlbumId>,

    /// The length of this track
    pub length: Duration,
    /// The track number of this track
    pub track_number: Option<u32>,
}

impl DatabaseStore {
    pub fn get_track(&self, id: TrackId) -> Result<&TrackData, MissingDatabaseId> {
        self.tracks.get(id).ok_or(MissingDatabaseId::Track(id))
    }

    pub fn insert_track(&mut self, track: TrackData) -> TrackId {
        self.tracks.insert_with_key(|id| {
            if let Some(album_id) = track.album
                && let Some(album) = self.albums.get_mut(album_id)
                && !album.derived.tracks.contains(&id)
            {
                album.derived.tracks.push(id);
            }

            if let Some(song_id) = track.song
                && let Some(song) = self.songs.get_mut(song_id)
                && !song.derived.tracks.contains(&id)
            {
                song.derived.tracks.push(id);
            }

            track
        })
    }
}
