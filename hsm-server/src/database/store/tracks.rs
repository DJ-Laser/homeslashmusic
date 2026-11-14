use std::time::Duration;

use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;

use crate::database::store::{
    DatabaseStore, MissingDatabaseId, ResourceLocation, SongData, SongId, TrackGroupData,
    TrackGroupId,
};

slotmap::new_key_type! {
    pub struct TrackId;
}

#[derive(Debug, Clone, Default)]
pub struct DerivedTrackData {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackData {
    /// The location of this track's audio data
    pub location: ResourceLocation,
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
}

impl DatabaseStore {
    pub fn get_track(&self, id: TrackId) -> Result<&TrackData, MissingDatabaseId> {
        self.tracks.get(id).ok_or(MissingDatabaseId::Track(id))
    }

    pub fn lookup_track_by_location(&self, location: &ResourceLocation) -> Option<TrackId> {
        self.track_by_location.get(location).copied()
    }

    pub(super) fn update_track_links(
        songs: &mut DenseSlotMap<SongId, SongData>,
        track_groups: &mut DenseSlotMap<TrackGroupId, TrackGroupData>,
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
    }

    fn insert_track(&mut self, track: TrackData) -> TrackId {
        self.tracks.insert_with_key(|id| {
            self.track_by_location.insert(track.location.clone(), id);

            Self::update_track_links(&mut self.songs, &mut self.track_groups, id, &track, false);

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

        Self::update_track_links(&mut self.songs, &mut self.track_groups, id, db_track, true);
        Self::update_track_links(&mut self.songs, &mut self.track_groups, id, &track, false);

        db_track.song = track.song;
        db_track.track_group = track.track_group;

        db_track.title = track.title;
        db_track.length = track.length;
        db_track.track_number = track.track_number;

        Ok(id)
    }
}
