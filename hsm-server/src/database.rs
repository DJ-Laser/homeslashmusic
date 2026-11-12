mod data;
pub mod input;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;

pub use data::*;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LookupMaps {
    track_by_audio_location: HashMap<AudioLocation, TrackId>,
    track_by_mbid: HashMap<MusicBrainzId, TrackId>,
    album_by_mbid: HashMap<MusicBrainzId, AlbumId>,
    artist_by_mbid: HashMap<MusicBrainzId, ArtistId>,
}

impl LookupMaps {
    fn update_track(&mut self, id: TrackId, track: &TrackData) {
        if let Some(mbid) = track.mbid {
            self.track_by_mbid.insert(mbid, id);
        }

        for location in &track.audio_locations {
            self.track_by_audio_location.insert(location.clone(), id);
        }
    }

    fn update_album(&mut self, id: AlbumId, album: &AlbumData) {
        if let Some(mbid) = album.mbid {
            self.album_by_mbid.insert(mbid, id);
        }
    }

    fn update_artist(&mut self, id: ArtistId, artist: &ArtistData) {
        if let Some(mbid) = artist.mbid {
            self.artist_by_mbid.insert(mbid, id);
        }
    }

    fn update_playlist(&mut self, id: PlaylistId, playlist: &PlaylistData) {
        let _ = id;
        let _ = playlist;
    }
}

#[derive(Error, Debug)]
pub enum MissingDatabaseId {
    #[error("Database does not contain key {0:?}")]
    Track(TrackId),
    #[error("Database does not contain key {0:?}")]
    Album(AlbumId),
    #[error("Database does not contain key {0:?}")]
    Artist(ArtistId),
    #[error("Database does not contain key {0:?}")]
    Playlist(PlaylistId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    tracks: DenseSlotMap<TrackId, TrackData>,
    albums: DenseSlotMap<AlbumId, AlbumData>,
    artists: DenseSlotMap<ArtistId, ArtistData>,
    playlists: DenseSlotMap<PlaylistId, PlaylistData>,

    lookup: LookupMaps,
}

impl Database {
    pub fn get_track(&self, id: TrackId) -> Result<&TrackData, MissingDatabaseId> {
        self.tracks.get(id).ok_or(MissingDatabaseId::Track(id))
    }

    pub fn insert_track(&mut self, track: TrackData) -> TrackId {
        self.tracks.insert_with_key(|id| {
            self.lookup.update_track(id, &track);
            track
        })
    }

    pub fn get_album(&self, id: AlbumId) -> Result<&AlbumData, MissingDatabaseId> {
        self.albums.get(id).ok_or(MissingDatabaseId::Album(id))
    }

    pub fn insert_album(&mut self, album: AlbumData) -> AlbumId {
        self.albums.insert_with_key(|id| {
            self.lookup.update_album(id, &album);
            album
        })
    }

    pub fn get_artist(&self, id: ArtistId) -> Result<&ArtistData, MissingDatabaseId> {
        self.artists.get(id).ok_or(MissingDatabaseId::Artist(id))
    }

    pub fn insert_artist(&mut self, artist: ArtistData) -> ArtistId {
        self.artists.insert_with_key(|id| {
            self.lookup.update_artist(id, &artist);
            artist
        })
    }

    pub fn get_playlist(&self, id: PlaylistId) -> Result<&PlaylistData, MissingDatabaseId> {
        self.playlists
            .get(id)
            .ok_or(MissingDatabaseId::Playlist(id))
    }

    pub fn insert_playlist(&mut self, playlist: PlaylistData) -> PlaylistId {
        self.playlists.insert_with_key(|id| {
            self.lookup.update_playlist(id, &playlist);
            playlist
        })
    }
}
