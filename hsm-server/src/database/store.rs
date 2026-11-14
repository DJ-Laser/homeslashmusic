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

use crate::database::sources::{
    ArtistLocation, PlaylistLocation, TrackGroupLocation, TrackLocation,
};

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

#[derive(Debug, Default)]
struct Lookup {
    track: HashMap<TrackLocation, TrackId>,
    track_group: HashMap<TrackGroupLocation, TrackGroupId>,
    playlist: HashMap<PlaylistLocation, PlaylistId>,

    song: HashMap<MusicBrainzId, SongId>,
    album: HashMap<MusicBrainzId, AlbumId>,
    artist_by_mbid: HashMap<MusicBrainzId, ArtistId>,
    artist_by_location: HashMap<ArtistLocation, ArtistId>,
}

impl Lookup {
    fn clear(&mut self) {
        self.track.clear();
        self.track_group.clear();
        self.playlist.clear();
        self.song.clear();
        self.album.clear();
        self.artist_by_mbid.clear();
        self.artist_by_location.clear();
    }
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
    lookup: Lookup,

    unqueried_mbids: HashSet<MusicBrainzId>,
    failed_mbids: HashSet<MusicBrainzId>,
}

impl DatabaseStore {
    pub fn rebuild_derrived(&mut self) {
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
            self.lookup.insert_track(track.location.clone(), id);
            Self::update_track_links(&mut self.songs, &mut self.track_groups, id, track, false);
        }

        for (id, track_group) in &self.track_groups {
            self.lookup
                .insert_track_group(track_group.location.clone(), id);
            Self::update_track_group_links(&mut self.albums, id, track_group, false);
        }

        for (id, playlist) in &self.playlists {
            self.lookup.insert_playlist(playlist.location.clone(), id);
        }

        for (id, song) in &self.songs {
            self.lookup.insert_song(song.mbid, id);
            Self::update_song_links(&mut self.artists, id, song, false);
        }

        for (id, album) in &self.albums {
            self.lookup.insert_album(album.mbid, id);
            Self::update_album_links(&mut self.artists, id, album, false);
        }

        for (id, artist) in &self.artists {
            self.lookup
                .insert_artist(artist.mbid, &artist.locations, id);
        }
    }
}
