mod albums;
mod artists;
mod mbid;
mod playlists;
mod songs;
mod track_groups;
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
pub use track_groups::*;
pub use tracks::*;

#[derive(Error, Debug)]
pub enum MissingDatabaseId {
    #[error("Database does not contain key {0:?}")]
    Track(TrackId),
    #[error("Database does not contain key {0:?}")]
    TrackGroup(TrackGroupId),
    #[error("Database does not contain key {0:?}")]
    Song(SongId),
    #[error("Database does not contain key {0:?}")]
    Album(AlbumId),
    #[error("Database does not contain key {0:?}")]
    Artist(ArtistId),
    #[error("Database does not contain key {0:?}")]
    Playlist(PlaylistId),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DatabaseStore {
    tracks: DenseSlotMap<TrackId, TrackData>,
    track_groups: DenseSlotMap<TrackGroupId, TrackGroupData>,
    playlists: DenseSlotMap<PlaylistId, PlaylistData>,

    songs: DenseSlotMap<SongId, SongData>,
    albums: DenseSlotMap<AlbumId, AlbumData>,
    artists: DenseSlotMap<ArtistId, ArtistData>,

    #[serde(skip)]
    track_by_location: HashMap<ResourceLocation, TrackId>,
    #[serde(skip)]
    track_group_by_location: HashMap<ResourceLocation, TrackGroupId>,
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
        self.track_group_by_location.clear();
        self.album_by_mbid.clear();
        self.artist_by_mbid.clear();

        for (_, song) in &mut self.songs {
            song.derived.clear();
        }
        for (_, track_group) in &mut self.track_groups {
            track_group.derived.clear();
        }
        for (_, album) in &mut self.albums {
            album.derived.clear();
        }
        for (_, artist) in &mut self.artists {
            artist.derived.clear();
        }

        for (id, track) in &self.tracks {
            self.track_by_location.insert(track.location.clone(), id);
            Self::update_track_links(&mut self.songs, &mut self.track_groups, id, track, false);
        }

        for (id, song) in &self.songs {
            self.song_by_mbid.insert(song.mbid, id);
            Self::update_song_links(&mut self.artists, id, song, false);
        }

        for (id, track) in &self.track_groups {
            self.track_group_by_location
                .insert(track.location.clone(), id);
            Self::update_track_group_links(&mut self.albums, id, track, false);
        }

        for (id, album) in &self.albums {
            self.album_by_mbid.insert(album.mbid, id);
            Self::update_album_links(&mut self.artists, id, album, false);
        }

        for (id, artist) in &self.artists {
            if let Some(mbid) = artist.mbid {
                self.artist_by_mbid.insert(mbid, id);
            }
        }
    }
}
