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

    pending_mbids: HashSet<MusicBrainzId>,
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

    pub fn print_contents(&self) {
        println!("##### TRACKS #####");
        println!();
        for (_, track) in &self.tracks {
            println!(
                "{} - {}",
                track.title.as_deref().unwrap_or("Unknown Track"),
                track.artists_text.as_deref().unwrap_or("Unknown Artist")
            );
        }
        println!();

        println!("##### TRACK GROUPS #####");
        println!();
        for (_, track_group) in &self.track_groups {
            println!(
                "{} - {} ({} tracks)",
                track_group
                    .title
                    .as_deref()
                    .unwrap_or("Unknown Track Group"),
                track_group
                    .artists_text
                    .as_deref()
                    .unwrap_or("Unknown Artist"),
                track_group.track_count.unwrap_or_default()
            );
        }
        println!();

        println!("##### SONGS #####");
        println!();
        for (_, song) in &self.songs {
            println!(
                "{} - {}, ({} mixed songs)",
                song.title.as_deref().unwrap_or("Unknown Song"),
                song.artists_text.as_deref().unwrap_or("Unknown Artist"),
                song.mix_songs.len()
            );
        }
        println!();

        println!("##### ALBUMS #####");
        println!();
        for (_, album) in &self.albums {
            println!(
                "{} - {}, ({} track groups)",
                album.title.as_deref().unwrap_or("Unknown Album"),
                album.artists_text.as_deref().unwrap_or("Unknown Artist"),
                album.derived.track_groups.len()
            );
        }
        println!();

        println!("##### ARTISTS #####");
        println!();
        for (_, artist) in &self.artists {
            println!(
                "{}, ({} songs and {} albums)",
                artist.name.as_deref().unwrap_or("Unknown Artist"),
                artist.derived.songs.len(),
                artist.derived.albums.len(),
            );
        }
        println!();

        println!("##### PENDING MBIDS #####");
        println!();
        for mbid in &self.pending_mbids {
            println!("{mbid}");
        }
    }
}
