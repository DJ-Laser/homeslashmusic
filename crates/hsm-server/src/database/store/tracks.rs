use std::{mem, time::Duration};

use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;

use crate::database::{
    sources::TrackLocation,
    store::{
        ArtistData, ArtistId, DatabaseStore, Lookup, MissingDatabaseId, SongData, SongId,
        TrackGroupData, TrackGroupId,
    },
};

slotmap::new_key_type! {
    pub struct TrackId;
}

#[derive(Debug, Clone, Default)]
pub struct DerivedTrackData {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackData {
    /// The location of this track's audio data
    pub location: TrackLocation,
    /// Track data derrived other database entries
    #[serde(skip)]
    pub derived: DerivedTrackData,

    /// The song this track is audio for
    pub song: Option<SongId>,
    // The track group this track is in
    pub track_group: Option<TrackGroupId>,

    /// The name of this track
    pub title: Option<String>,
    /// The length of this track
    pub length: Duration,
    /// The track number of this track
    pub track_number: Option<u32>,

    /// Artists assocated with this track, sorted as they appear on the track metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the track's artists. May include separators like "feat."
    pub artists_text: Option<String>,
}

impl DatabaseStore {
    pub fn get_track(&self, id: TrackId) -> Result<&TrackData, MissingDatabaseId> {
        self.tracks.get(id).ok_or(MissingDatabaseId::Track(id))
    }

    pub fn lookup_track_by_location(&self, location: &TrackLocation) -> Option<TrackId> {
        self.lookup.track.get(location).copied()
    }

    pub(super) fn update_track_links(
        songs: &mut DenseSlotMap<SongId, SongData>,
        track_groups: &mut DenseSlotMap<TrackGroupId, TrackGroupData>,
        artists: &mut DenseSlotMap<ArtistId, ArtistData>,
        id: TrackId,
        track: &TrackData,
        remove: bool,
    ) {
        if let Some(song_id) = track.song
            && let Some(song) = songs.get_mut(song_id)
        {
            if remove {
                song.derived.tracks.retain(|stored_id| *stored_id != id);
            } else if !song.derived.tracks.contains(&id) {
                song.derived.tracks.push(id);
            }
        }

        if let Some(track_group_id) = track.track_group
            && let Some(track_group) = track_groups.get_mut(track_group_id)
        {
            if remove {
                track_group
                    .derived
                    .tracks
                    .retain(|stored_id| *stored_id != id);
            } else if !track_group.derived.tracks.contains(&id) {
                track_group.derived.tracks.push(id);
            }
        }

        for artist_id in &track.artists {
            let Some(artist) = artists.get_mut(*artist_id) else {
                continue;
            };

            if remove {
                artist.derived.tracks.retain(|stored_id| *stored_id != id);
            } else if !artist.derived.tracks.contains(&id) {
                artist.derived.tracks.push(id);
            }
        }
    }

    fn insert_track(&mut self, track: TrackData) -> TrackId {
        self.tracks.insert_with_key(|id| {
            self.lookup.insert_track(track.location.clone(), id);

            Self::update_track_links(
                &mut self.songs,
                &mut self.track_groups,
                &mut self.artists,
                id,
                &track,
                false,
            );

            track
        })
    }

    pub fn update_or_insert_track(
        &mut self,
        track: TrackData,
    ) -> Result<TrackId, MissingDatabaseId> {
        let Some(id) = self.lookup_track_by_location(&track.location) else {
            return Ok(self.insert_track(track));
        };

        let db_track = self
            .tracks
            .get_mut(id)
            .ok_or(MissingDatabaseId::Track(id))?;

        Self::update_track_links(
            &mut self.songs,
            &mut self.track_groups,
            &mut self.artists,
            id,
            db_track,
            true,
        );

        *db_track = TrackData {
            location: mem::replace(&mut db_track.location, track.location),
            derived: mem::replace(&mut db_track.derived, track.derived),
            ..track
        };

        Self::update_track_links(
            &mut self.songs,
            &mut self.track_groups,
            &mut self.artists,
            id,
            db_track,
            false,
        );

        Ok(id)
    }
}

impl Lookup {
    pub fn insert_track(&mut self, location: TrackLocation, id: TrackId) {
        self.track.insert(location, id);
    }
}
