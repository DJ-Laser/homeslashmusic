use std::mem;

use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;

use crate::database::{
    sources::TrackGroupLocation,
    store::{AlbumData, AlbumId, ArtistId, DatabaseStore, Lookup, MissingDatabaseId, TrackId},
};

slotmap::new_key_type! {
    pub struct TrackGroupId;
}

#[derive(Debug, Clone, Default)]
pub struct DerivedTrackGroupData {
    pub(super) tracks: Vec<TrackId>,
}

impl DerivedTrackGroupData {
    /// Tracks in this track group
    pub fn songs(&self) -> &Vec<TrackId> {
        &self.tracks
    }

    pub(super) fn clear(&mut self) {
        self.tracks.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackGroupData {
    /// The location of this track group
    pub location: TrackGroupLocation,
    /// Track group data derrived other database entries
    #[serde(skip)]
    pub derived: DerivedTrackGroupData,

    /// The album this track group is for
    pub album: Option<AlbumId>,

    /// Name of the track group
    pub title: Option<String>,
    /// The total number of tracks included in this track group
    pub track_count: Option<u32>,

    /// Artists assocated with this track group, sorted as they appear on the track group metadata
    pub artists: Vec<ArtistId>,
    /// The string representation of the track group's artists. May include separators like "feat."
    pub artists_text: Option<String>,
}

impl DatabaseStore {
    pub fn get_track_group(&self, id: TrackGroupId) -> Result<&TrackGroupData, MissingDatabaseId> {
        self.track_groups
            .get(id)
            .ok_or(MissingDatabaseId::TrackGroup(id))
    }

    pub fn lookup_track_group(&self, location: &TrackGroupLocation) -> Option<TrackGroupId> {
        self.lookup.track_group.get(location).copied()
    }

    pub fn insert_track_group(&mut self, track_group: TrackGroupData) -> TrackGroupId {
        self.track_groups.insert_with_key(|id| {
            self.lookup
                .insert_track_group(track_group.location.clone(), id);

            Self::update_track_group_links(&mut self.albums, id, &track_group, false);

            track_group
        })
    }

    pub(super) fn update_track_group_links(
        albums: &mut DenseSlotMap<AlbumId, AlbumData>,
        id: TrackGroupId,
        track_group: &TrackGroupData,
        remove: bool,
    ) {
        if let Some(album_id) = track_group.album
            && let Some(album) = albums.get_mut(album_id)
        {
            if remove {
                album
                    .derived
                    .track_groups
                    .retain(|stored_id| *stored_id != id);
            } else if !album.derived.track_groups.contains(&id) {
                album.derived.track_groups.push(id);
            }
        }
    }

    pub fn update_or_insert_track_group(
        &mut self,
        track_group: TrackGroupData,
    ) -> Result<TrackGroupId, MissingDatabaseId> {
        let Some(id) = self.lookup_track_group(&track_group.location) else {
            return Ok(self.insert_track_group(track_group));
        };

        let db_track_group = self
            .track_groups
            .get_mut(id)
            .ok_or(MissingDatabaseId::TrackGroup(id))?;

        Self::update_track_group_links(&mut self.albums, id, db_track_group, true);

        *db_track_group = TrackGroupData {
            location: mem::replace(&mut db_track_group.location, track_group.location),
            derived: mem::replace(&mut db_track_group.derived, track_group.derived),
            ..track_group
        };

        Self::update_track_group_links(&mut self.albums, id, db_track_group, false);

        Ok(id)
    }
}

impl Lookup {
    pub fn insert_track_group(&mut self, location: TrackGroupLocation, id: TrackGroupId) {
        self.track_group.insert(location, id);
    }
}
