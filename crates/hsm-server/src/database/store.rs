mod albums;
mod artists;
mod insertion;
mod ipc;
mod mbid;
mod playlists;
mod search;
mod songs;
mod track_groups;
mod tracks;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slotmap::DenseSlotMap;
use thiserror::Error;

pub use albums::*;
pub use artists::*;
pub use mbid::*;
pub use playlists::*;
pub use search::*;
pub use songs::*;
pub use track_groups::*;
pub use tracks::*;

use crate::database::sources::{
    ArtistLocation, PlaylistLocation, TrackGroupLocation, TrackLocation, UpdateState,
};

#[derive(Error, Debug)]
pub enum MissingDatabaseId {
    #[error("Database does not contain track {0}")]
    Track(TrackId),
    #[error("Database does not contain track group {0}")]
    TrackGroup(TrackGroupId),
    #[error("Database does not contain song {0}")]
    Song(SongId),
    #[error("Database does not contain album {0}")]
    Album(AlbumId),
    #[error("Database does not contain artist {0}")]
    Artist(ArtistId),
    #[error("Database does not contain playlist {0}")]
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
    mbid_query_state: MbidQueryState,
}

impl DatabaseStore {
    pub fn rebuild_derived(&mut self) {
        self.lookup.clear();

        for track in self.tracks.values_mut() {
            track.derived.clear();
        }
        for song in self.songs.values_mut() {
            song.derived.clear();
        }
        for track_group in self.track_groups.values_mut() {
            track_group.derived.clear();
        }
        for album in self.albums.values_mut() {
            album.derived.clear();
        }
        for artist in self.artists.values_mut() {
            artist.derived.clear();
        }
        for playlist in self.playlists.values_mut() {
            playlist.derived.clear();
        }

        for (id, track) in &self.tracks {
            self.lookup.insert_track(track.location.clone(), id);
            Self::update_track_links(
                &mut self.songs,
                &mut self.track_groups,
                &mut self.artists,
                id,
                track,
                false,
            );
        }

        for (id, track_group) in &self.track_groups {
            self.lookup
                .insert_track_group(track_group.location.clone(), id);
            Self::update_track_group_links(
                &mut self.albums,
                &mut self.artists,
                id,
                track_group,
                false,
            );
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

        let mut playlist_ids = Vec::with_capacity(self.playlists.len());
        for (id, playlist) in &self.playlists {
            self.lookup.insert_playlist(playlist.location.clone(), id);
            playlist_ids.push(id);
        }

        for id in playlist_ids {
            Self::update_playlist_links(
                &mut self.tracks,
                &mut self.track_groups,
                &mut self.songs,
                &mut self.albums,
                &mut self.playlists,
                id,
                false,
            );
        }
    }

    pub fn print_contents(&self) {
        println!();
        println!("##### TRACKS #####");
        println!();
        for (_, track) in &self.tracks {
            println!(
                "{} - {}",
                track.title,
                track.artists_text.as_deref().unwrap_or("Unknown Artist")
            );
        }

        println!();
        println!("##### TRACK GROUPS #####");
        println!();
        for (_, track_group) in &self.track_groups {
            println!(
                "{} - {} ({} tracks)",
                track_group.title,
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
                song.title,
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
                album.title,
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
                artist.name,
                artist.derived.songs.len(),
                artist.derived.albums.len(),
            );
        }

        println!();
        println!("##### INCOMPLETE MBIDS #####");
        println!();
        for mbid in self.mbid_query_state.incomplete.keys() {
            if self.mbid_query_state.pending.contains_key(mbid) {
                println!("{mbid} (pending)");
            } else {
                println!("{mbid}");
            }
        }
        for mbid in self.mbid_query_state.failed.keys() {
            println!("{mbid} (failed)");
        }

        println!();
    }

    pub fn purge_post_update(&mut self, state: &UpdateState) {
        let _ = state;
        let _ = self;
        //TODO
    }
}
