use std::{fmt::Display, mem};

use log::error;
use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;

use crate::database::{
    sources::{PlaylistLocation, ProviderId},
    store::{
        AlbumData, AlbumId, DatabaseStore, Lookup, MissingDatabaseId, SongData, SongId, TrackData,
        TrackGroupData, TrackGroupId, TrackId,
    },
};

slotmap::new_key_type! {
    pub struct PlaylistId;
}

impl Display for PlaylistId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaylistEntry {
    /// Play the specified track
    Track(TrackId),
    /// Play the preferred track for the song
    Song(SongId),
    /// Play the specified track group
    TrackGroup(TrackGroupId),
    /// Play the preferred track group for the album
    Album(AlbumId),
    /// Play the contents of the playlist
    Playlist(PlaylistId),
}

#[derive(Debug, Clone, Default)]
pub struct DerivedPlaylistData {
    pub(super) in_playlists: Vec<PlaylistId>,
}

impl DerivedPlaylistData {
    /// Playlists this playlist is in
    pub fn in_playlists(&self) -> &Vec<PlaylistId> {
        &self.in_playlists
    }

    pub fn clear(&mut self) {
        self.in_playlists.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistData {
    /// The name of the this playlist's provider
    pub provider: ProviderId,
    /// The location of this playlist
    pub location: PlaylistLocation,
    /// All entries in the playlist, sorted by play order
    pub entries: Vec<PlaylistEntry>,
    /// Playlist data derrived other database entries
    #[serde(skip)]
    pub derived: DerivedPlaylistData,

    /// Name of the playlist
    pub title: String,
    /// The genres associated with this playlist, sorted by relevance
    pub genres: Vec<String>,
}

impl DatabaseStore {
    pub fn get_playlist(&self, id: PlaylistId) -> Result<&PlaylistData, MissingDatabaseId> {
        self.playlists
            .get(id)
            .ok_or(MissingDatabaseId::Playlist(id))
    }

    pub fn lookup_playlist(&self, location: &PlaylistLocation) -> Option<PlaylistId> {
        self.lookup.playlist.get(location).copied()
    }

    fn insert_playlist(&mut self, playlist: PlaylistData) -> PlaylistId {
        let id = self.playlists.insert_with_key(|id| {
            self.lookup.insert_playlist(playlist.location.clone(), id);

            playlist
        });

        Self::update_playlist_links(
            &mut self.tracks,
            &mut self.track_groups,
            &mut self.songs,
            &mut self.albums,
            &mut self.playlists,
            id,
            false,
        );

        id
    }

    pub(super) fn update_playlist_links(
        tracks: &mut DenseSlotMap<TrackId, TrackData>,
        track_groups: &mut DenseSlotMap<TrackGroupId, TrackGroupData>,
        songs: &mut DenseSlotMap<SongId, SongData>,
        albums: &mut DenseSlotMap<AlbumId, AlbumData>,
        playlists: &mut DenseSlotMap<PlaylistId, PlaylistData>,
        id: PlaylistId,
        remove: bool,
    ) {
        let update_links = |in_playlists: &mut Vec<PlaylistId>| {
            if remove {
                in_playlists.retain(|stored_id| *stored_id != id);
            } else if !in_playlists.contains(&id) {
                in_playlists.push(id);
            }
        };

        let Some(entries) = playlists
            .get_mut(id)
            .map(|playlist| mem::take(&mut playlist.entries))
        else {
            return;
        };

        for entry in &entries {
            match *entry {
                PlaylistEntry::Track(track_id) => {
                    if let Some(track) = tracks.get_mut(track_id) {
                        update_links(&mut track.derived.in_playlists);
                    }
                }
                PlaylistEntry::Song(song_id) => {
                    if let Some(song) = songs.get_mut(song_id) {
                        update_links(&mut song.derived.in_playlists);
                    }
                }
                PlaylistEntry::TrackGroup(track_group_id) => {
                    if let Some(track_group) = track_groups.get_mut(track_group_id) {
                        update_links(&mut track_group.derived.in_playlists);
                    }
                }
                PlaylistEntry::Album(album_id) => {
                    if let Some(album) = albums.get_mut(album_id) {
                        update_links(&mut album.derived.in_playlists);
                    }
                }
                PlaylistEntry::Playlist(playlist_id) => {
                    if playlist_id != id
                        && let Some(playlist) = playlists.get_mut(playlist_id)
                    {
                        update_links(&mut playlist.derived.in_playlists);
                    }
                }
            }
        }

        let Some(playlist) = playlists.get_mut(id) else {
            return;
        };

        playlist.entries = entries;
    }

    pub fn update_or_insert_playlist(&mut self, playlist: PlaylistData) -> PlaylistId {
        let Some(id) = self.lookup_playlist(&playlist.location) else {
            return self.insert_playlist(playlist);
        };

        Self::update_playlist_links(
            &mut self.tracks,
            &mut self.track_groups,
            &mut self.songs,
            &mut self.albums,
            &mut self.playlists,
            id,
            true,
        );

        let Some(db_playlist) = self.playlists.get_mut(id) else {
            error!(
                "Error updating playlist with location {}: {}",
                playlist.location,
                MissingDatabaseId::Playlist(id)
            );

            return self.insert_playlist(playlist);
        };

        let old_playlist = mem::replace(db_playlist, playlist);

        db_playlist.derived = old_playlist.derived;

        Self::update_playlist_links(
            &mut self.tracks,
            &mut self.track_groups,
            &mut self.songs,
            &mut self.albums,
            &mut self.playlists,
            id,
            false,
        );

        id
    }

    pub fn remove_playlist(&mut self, id: PlaylistId) {
        Self::update_playlist_links(
            &mut self.tracks,
            &mut self.track_groups,
            &mut self.songs,
            &mut self.albums,
            &mut self.playlists,
            id,
            true,
        );

        if let Some(playlist) = self.playlists.remove(id) {
            self.lookup.playlist.remove(&playlist.location);
        }
    }
}

impl Lookup {
    pub fn insert_playlist(&mut self, location: PlaylistLocation, id: PlaylistId) {
        self.playlist.insert(location, id);
    }
}
