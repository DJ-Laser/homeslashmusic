mod albums;
mod artists;
mod mbid;
mod playlists;
mod songs;
mod tracks;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;
use thiserror::Error;

pub use albums::*;
pub use artists::*;
pub use mbid::*;
pub use playlists::*;
pub use songs::*;
pub use tracks::*;

#[derive(Error, Debug)]
pub enum MissingDatabaseId {
    #[error("Database does not contain key {0:?}")]
    Song(SongId),
    #[error("Database does not contain key {0:?}")]
    Track(TrackId),
    #[error("Database does not contain key {0:?}")]
    Album(AlbumId),
    #[error("Database does not contain key {0:?}")]
    Artist(ArtistId),
    #[error("Database does not contain key {0:?}")]
    Playlist(PlaylistId),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DatabaseStore {
    songs: DenseSlotMap<SongId, SongData>,
    tracks: DenseSlotMap<TrackId, TrackData>,
    albums: DenseSlotMap<AlbumId, AlbumData>,
    artists: DenseSlotMap<ArtistId, ArtistData>,
    playlists: DenseSlotMap<PlaylistId, PlaylistData>,

    #[serde(skip)]
    track_by_location: HashMap<ResourceLocation, TrackId>,
    #[serde(skip)]
    song_by_mbid: HashMap<MusicBrainzId, SongId>,
    #[serde(skip)]
    album_by_mbid: HashMap<MusicBrainzId, AlbumId>,
    #[serde(skip)]
    artist_by_mbid: HashMap<MusicBrainzId, ArtistId>,

    unqueried_mbids: HashSet<MusicBrainzId>,
    failed_mbids: HashSet<MusicBrainzId>,
}

impl DatabaseStore {
    pub fn rebuild_derrived(&mut self) {
        self.track_by_location.clear();
        self.song_by_mbid.clear();
        self.album_by_mbid.clear();
        self.artist_by_mbid.clear();

        for (_, song) in &mut self.songs {
            song.derived.clear();
        }
        for (_, album) in &mut self.albums {
            album.derived.clear();
        }
        for (_, artist) in &mut self.artists {
            artist.derived.clear();
        }

        for (id, track) in &self.tracks {
            self.track_by_location.insert(track.location.clone(), id);
            Self::update_track_links(&mut self.songs, &mut self.albums, id, track, false);
        }
        for (id, song) in &self.songs {
            if let Some(mbid) = song.mbid {
                self.song_by_mbid.insert(mbid, id);
            }
            Self::update_song_links(&mut self.artists, id, song, false);
        }
        for (id, album) in &self.albums {
            if let Some(mbid) = album.mbid {
                self.album_by_mbid.insert(mbid, id);
            }
            Self::update_album_links(&mut self.artists, id, album, false);
        }
        for (id, artist) in &self.artists {
            if let Some(mbid) = artist.mbid {
                self.artist_by_mbid.insert(mbid, id);
            }
        }
    }
}
