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
    ArtistLocation, PlaylistLocation, ProviderId, TrackGroupLocation, TrackLocation, UpdateState,
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

        let mut song_ids = Vec::with_capacity(self.songs.len());
        for (id, song) in &self.songs {
            self.lookup.insert_song(song.mbid, id);
            Self::update_song_links(&mut self.artists, id, song, false);
            song_ids.push(id);
        }

        for id in song_ids {
            Self::update_song_relationship_links(&mut self.songs, id, false);
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
                "{} - {} ({} related songs)",
                song.title,
                song.artists_text.as_deref().unwrap_or("Unknown Artist"),
                song.relationships.values().map(Vec::len).sum::<usize>()
            );
        }

        println!();
        println!("##### ALBUMS #####");
        println!();
        for (_, album) in &self.albums {
            println!(
                "{} - {} ({} track groups)",
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
                "{} ({} songs and {} albums)",
                artist.name,
                artist.derived.songs.len(),
                artist.derived.albums.len(),
            );
        }

        println!();
        println!("##### INCOMPLETE MBIDS #####");
        println!();
        println!("{} incomplete", self.mbid_query_state.incomplete.len());
        println!("{} pending", self.mbid_query_state.pending.len());
        println!("{} failed", self.mbid_query_state.failed.len());

        println!();
    }

    pub fn purge_orphaned_entities(&mut self) {
        // Remove songs in a loop to catch long relationship chains
        loop {
            let remove_songs: Vec<_> = self
                .songs
                .iter()
                .filter_map(|(id, song)| {
                    if song.derived.tracks.is_empty()
                        && song.derived.in_playlists.is_empty()
                        && song.derived.related_to.is_empty()
                    {
                        return Some(id);
                    }

                    None
                })
                .collect();

            if remove_songs.is_empty() {
                break;
            }

            for id in remove_songs {
                self.remove_song(id);
            }
        }

        let remove_albums: Vec<_> = self
            .albums
            .iter()
            .filter_map(|(id, album)| {
                if album.derived.track_groups.is_empty() && album.derived.in_playlists.is_empty() {
                    return Some(id);
                }

                None
            })
            .collect();

        for id in remove_albums {
            self.remove_album(id);
        }

        let remove_artists: Vec<_> = self
            .artists
            .iter()
            .filter_map(|(id, artist)| {
                if artist.derived.tracks.is_empty()
                    && artist.derived.track_groups.is_empty()
                    && artist.derived.songs.is_empty()
                    && artist.derived.albums.is_empty()
                {
                    return Some(id);
                }

                None
            })
            .collect();

        for id in remove_artists {
            self.remove_artist(id);
        }

        self.check_integrity();
    }

    pub fn purge_post_update(&mut self, state: &UpdateState) {
        let provider_excluded = |provider: &ProviderId| {
            if let Some(ref updated_providers) = state.updated_providers
                && !updated_providers.contains(provider)
            {
                return true;
            }

            state.failed_providers.contains(provider)
        };

        let remove_playlists: Vec<_> = self
            .playlists
            .iter()
            .filter_map(|(id, playlist)| {
                if provider_excluded(&playlist.provider) {
                    return None;
                }

                if !state.seen_playlists.contains_key(&playlist.location) {
                    return Some(id);
                }

                None
            })
            .collect();

        for id in remove_playlists {
            self.remove_playlist(id);
        }

        let remove_tracks: Vec<_> = self
            .tracks
            .iter()
            .filter_map(|(id, track)| {
                if provider_excluded(&track.provider) {
                    return None;
                }

                if !state.seen_tracks.contains_key(&track.location) {
                    return Some(id);
                }

                None
            })
            .collect();

        for id in remove_tracks {
            self.remove_track(id);
        }

        let remove_track_groups: Vec<_> = self
            .track_groups
            .iter()
            .filter_map(|(id, track_group)| {
                if provider_excluded(&track_group.provider) {
                    return None;
                }

                if !state.seen_track_groups.contains_key(&track_group.location) {
                    return Some(id);
                }

                None
            })
            .collect();

        for id in remove_track_groups {
            self.remove_track_group(id);
        }

        self.purge_orphaned_entities();
    }
}

mod integrity {
    use log::error;

    use crate::database::store::{DatabaseStore, PlaylistEntry};

    #[derive(Debug, thiserror::Error)]
    #[error("{entity}: {reason}")]
    pub struct IntegrityError {
        pub entity: &'static str,
        pub reason: String,
    }

    impl DatabaseStore {
        /// Verifies all relational foreign keys, `derived` reverse lists, and `lookup` indexes.
        #[expect(clippy::too_many_lines, reason = "Test code")]
        fn find_integrity_errors(&self) -> Vec<IntegrityError> {
            let mut errors = Vec::new();
            let mut push_err = |entity: &'static str, reason: String| {
                errors.push(IntegrityError { entity, reason });
            };

            // 1. Tracks -> Song, TrackGroup, Artists, in_playlists + lookup.track
            for (track_id, track) in &self.tracks {
                if self.lookup.track.get(&track.location) != Some(&track_id) {
                    push_err(
                        "Track",
                        format!(
                            "{track_id} has missing or mismatched lookup.track entry for location {}",
                            track.location
                        ),
                    );
                }

                if let Some(song_id) = track.song {
                    match self.songs.get(song_id) {
                        None => push_err(
                            "Track",
                            format!("{track_id} references missing Song {song_id} in track.song"),
                        ),
                        Some(song) if !song.derived.tracks.contains(&track_id) => push_err(
                            "Track",
                            format!(
                                "{track_id} references Song {song_id} in track.song, but {song_id} does not list {track_id} in song.derived.tracks"
                            ),
                        ),
                        _ => {}
                    }
                }

                if let Some(group_id) = track.track_group {
                    match self.track_groups.get(group_id) {
                        None => push_err(
                            "Track",
                            format!(
                                "{track_id} references missing TrackGroup {group_id} in track.track_group"
                            ),
                        ),
                        Some(group) if !group.derived.tracks.contains(&track_id) => push_err(
                            "Track",
                            format!(
                                "{track_id} references TrackGroup {group_id} in track.track_group, but {group_id} does not list {track_id} in track_group.derived.tracks"
                            ),
                        ),
                        _ => {}
                    }
                }

                for &artist_id in &track.artists {
                    match self.artists.get(artist_id) {
                        None => push_err(
                            "Track",
                            format!(
                                "{track_id} references missing Artist {artist_id} in track.artists"
                            ),
                        ),
                        Some(artist) if !artist.derived.tracks.contains(&track_id) => push_err(
                            "Track",
                            format!(
                                "{track_id} references Artist {artist_id} in track.artists, but {artist_id} does not list {track_id} in artist.derived.tracks"
                            ),
                        ),
                        _ => {}
                    }
                }

                for &playlist_id in &track.derived.in_playlists {
                    match self.playlists.get(playlist_id) {
                        None => push_err(
                            "Track",
                            format!(
                                "{track_id} references missing Playlist {playlist_id} in track.derived.in_playlists"
                            ),
                        ),
                        Some(playlist)
                            if !playlist.entries.contains(&PlaylistEntry::Track(track_id)) =>
                        {
                            push_err(
                                "Track",
                                format!(
                                    "{track_id} lists Playlist {playlist_id} in track.derived.in_playlists, but {playlist_id} does not contain {track_id} in playlist.entries"
                                ),
                            );
                        }
                        _ => {}
                    }
                }
            }

            for (location, &track_id) in &self.lookup.track {
                if !self.tracks.contains_key(track_id) {
                    push_err(
                        "lookup.track",
                        format!("location {location} points to missing Track {track_id}"),
                    );
                }
            }

            // 2. TrackGroups -> Album, Artists, derived.tracks, in_playlists + lookup.track_group
            for (group_id, group) in &self.track_groups {
                if self.lookup.track_group.get(&group.location) != Some(&group_id) {
                    push_err(
                        "TrackGroup",
                        format!(
                            "{group_id} has missing or mismatched lookup.track_group entry for location {}",
                            group.location
                        ),
                    );
                }

                if let Some(album_id) = group.album {
                    match self.albums.get(album_id) {
                        None => push_err(
                            "TrackGroup",
                            format!(
                                "{group_id} references missing Album {album_id} in track_group.album"
                            ),
                        ),
                        Some(album) if !album.derived.track_groups.contains(&group_id) => push_err(
                            "TrackGroup",
                            format!(
                                "{group_id} references Album {album_id} in track_group.album, but {album_id} does not list {group_id} in album.derived.track_groups"
                            ),
                        ),
                        _ => {}
                    }
                }

                for &artist_id in &group.artists {
                    match self.artists.get(artist_id) {
                        None => push_err(
                            "TrackGroup",
                            format!(
                                "{group_id} references missing Artist {artist_id} in track_group.artists"
                            ),
                        ),
                        Some(artist) if !artist.derived.track_groups.contains(&group_id) => {
                            push_err(
                                "TrackGroup",
                                format!(
                                    "{group_id} references Artist {artist_id} in track_group.artists, but {artist_id} does not list {group_id} in artist.derived.track_groups"
                                ),
                            );
                        }
                        _ => {}
                    }
                }

                for &track_id in &group.derived.tracks {
                    match self.tracks.get(track_id) {
                        None => push_err(
                            "TrackGroup",
                            format!(
                                "{group_id} references missing Track {track_id} in track_group.derived.tracks"
                            ),
                        ),
                        Some(track) if track.track_group != Some(group_id) => push_err(
                            "TrackGroup",
                            format!(
                                "{group_id} lists Track {track_id} in track_group.derived.tracks, but {track_id} does not reference {group_id} in track.track_group"
                            ),
                        ),
                        _ => {}
                    }
                }

                for &playlist_id in &group.derived.in_playlists {
                    match self.playlists.get(playlist_id) {
                        None => push_err(
                            "TrackGroup",
                            format!(
                                "{group_id} references missing Playlist {playlist_id} in track_group.derived.in_playlists"
                            ),
                        ),
                        Some(playlist)
                            if !playlist
                                .entries
                                .contains(&PlaylistEntry::TrackGroup(group_id)) =>
                        {
                            push_err(
                                "TrackGroup",
                                format!(
                                    "{group_id} lists Playlist {playlist_id} in track_group.derived.in_playlists, but {playlist_id} does not contain {group_id} in playlist.entries"
                                ),
                            );
                        }
                        _ => {}
                    }
                }
            }

            for (location, &group_id) in &self.lookup.track_group {
                if !self.track_groups.contains_key(group_id) {
                    push_err(
                        "lookup.track_group",
                        format!("location {location} points to missing TrackGroup {group_id}"),
                    );
                }
            }

            // 3. Songs -> derived.tracks, Artists, in_playlists, relationships, derived.related_to + lookup.song
            for (song_id, song) in &self.songs {
                // Relationship targets can be listed for metadata purposes without playable tracks
                if song.derived.related_to.is_empty() && song.derived.tracks.is_empty() {
                    push_err(
                        "Song",
                        format!(
                            "{song_id} is orphaned with empty song.derived.tracks and empty song.derived.related_to"
                        ),
                    );
                }

                if self.lookup.song.get(&song.mbid) != Some(&song_id) {
                    push_err(
                        "Song",
                        format!(
                            "{song_id} has missing or mismatched lookup.song entry for MBID {}",
                            song.mbid
                        ),
                    );
                }

                for &track_id in &song.derived.tracks {
                    match self.tracks.get(track_id) {
                        None => push_err(
                            "Song",
                            format!(
                                "{song_id} references missing Track {track_id} in song.derived.tracks"
                            ),
                        ),
                        Some(track) if track.song != Some(song_id) => push_err(
                            "Song",
                            format!(
                                "{song_id} lists Track {track_id} in song.derived.tracks, but {track_id} does not reference {song_id} in track.song"
                            ),
                        ),
                        _ => {}
                    }
                }
                for &artist_id in &song.artists {
                    match self.artists.get(artist_id) {
                        None => push_err(
                            "Song",
                            format!(
                                "{song_id} references missing Artist {artist_id} in song.artists"
                            ),
                        ),
                        Some(artist) if !artist.derived.songs.contains(&song_id) => push_err(
                            "Song",
                            format!(
                                "{song_id} references Artist {artist_id} in song.artists, but {artist_id} does not list {song_id} in artist.derived.songs"
                            ),
                        ),
                        _ => {}
                    }
                }
                for &playlist_id in &song.derived.in_playlists {
                    match self.playlists.get(playlist_id) {
                        None => push_err(
                            "Song",
                            format!(
                                "{song_id} references missing Playlist {playlist_id} in song.derived.in_playlists"
                            ),
                        ),
                        Some(playlist)
                            if !playlist.entries.contains(&PlaylistEntry::Song(song_id)) =>
                        {
                            push_err(
                                "Song",
                                format!(
                                    "{song_id} lists Playlist {playlist_id} in song.derived.in_playlists, but {playlist_id} does not contain {song_id} in playlist.entries"
                                ),
                            );
                        }
                        _ => {}
                    }
                }
                for (kind, targets) in &song.relationships {
                    if targets.is_empty() {
                        push_err(
                            "Song",
                            format!(
                                "{song_id} has empty target list in song.relationships[{kind}]"
                            ),
                        );
                    }
                    for &target_id in targets {
                        if target_id == song_id {
                            push_err(
                                "Song",
                                format!(
                                    "{song_id} has self-referential {kind} entry in song.relationships"
                                ),
                            );
                        }
                        match self.songs.get(target_id) {
                            None => push_err(
                                "Song",
                                format!(
                                    "{song_id} references missing target Song {target_id} in song.relationships[{kind}]"
                                ),
                            ),
                            Some(target)
                                if !target
                                    .derived
                                    .related_to
                                    .get(kind)
                                    .is_some_and(|list| list.contains(&song_id)) =>
                            {
                                push_err(
                                    "Song",
                                    format!(
                                        "{song_id} references Song {target_id} in song.relationships[{kind}], but {target_id} does not list {song_id} in song.derived.related_to[{kind}]"
                                    ),
                                );
                            }
                            _ => {}
                        }
                    }
                }
                for (kind, sources) in &song.derived.related_to {
                    if sources.is_empty() {
                        push_err(
                            "Song",
                            format!(
                                "{song_id} has empty source list in song.derived.related_to[{kind}]"
                            ),
                        );
                    }
                    for &source_id in sources {
                        match self.songs.get(source_id) {
                            None => push_err(
                                "Song",
                                format!(
                                    "{song_id} references missing source Song {source_id} in song.derived.related_to[{kind}]"
                                ),
                            ),
                            Some(source)
                                if !source
                                    .relationships
                                    .get(kind)
                                    .is_some_and(|list| list.contains(&song_id)) =>
                            {
                                push_err(
                                    "Song",
                                    format!(
                                        "{song_id} lists Song {source_id} in song.derived.related_to[{kind}], but {source_id} does not list {song_id} in song.relationships[{kind}]"
                                    ),
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
            for (mbid, &song_id) in &self.lookup.song {
                if !self.songs.contains_key(song_id) {
                    push_err(
                        "lookup.song",
                        format!("MBID {mbid} points to missing Song {song_id}"),
                    );
                }
            }
            // 4. Albums -> derived.track_groups, Artists, in_playlists + lookup.album
            for (album_id, album) in &self.albums {
                if album.derived.track_groups.is_empty() && album.derived.in_playlists.is_empty() {
                    push_err(
                        "Album",
                        format!(
                            "{album_id} is orphaned with empty album.derived.track_groups and empty album.derived.in_playlists"
                        ),
                    );
                }
                if self.lookup.album.get(&album.mbid) != Some(&album_id) {
                    push_err(
                        "Album",
                        format!(
                            "{album_id} has missing or mismatched lookup.album entry for MBID {}",
                            album.mbid
                        ),
                    );
                }
                for &group_id in &album.derived.track_groups {
                    match self.track_groups.get(group_id) {
                        None => push_err(
                            "Album",
                            format!(
                                "{album_id} references missing TrackGroup {group_id} in album.derived.track_groups"
                            ),
                        ),
                        Some(group) if group.album != Some(album_id) => push_err(
                            "Album",
                            format!(
                                "{album_id} lists TrackGroup {group_id} in album.derived.track_groups, but {group_id} does not reference {album_id} in track_group.album"
                            ),
                        ),
                        _ => {}
                    }
                }
                for &artist_id in &album.artists {
                    match self.artists.get(artist_id) {
                        None => push_err(
                            "Album",
                            format!(
                                "{album_id} references missing Artist {artist_id} in album.artists"
                            ),
                        ),
                        Some(artist) if !artist.derived.albums.contains(&album_id) => push_err(
                            "Album",
                            format!(
                                "{album_id} references Artist {artist_id} in album.artists, but {artist_id} does not list {album_id} in artist.derived.albums"
                            ),
                        ),
                        _ => {}
                    }
                }
                for &playlist_id in &album.derived.in_playlists {
                    match self.playlists.get(playlist_id) {
                        None => push_err(
                            "Album",
                            format!(
                                "{album_id} references missing Playlist {playlist_id} in album.derived.in_playlists"
                            ),
                        ),
                        Some(playlist)
                            if !playlist.entries.contains(&PlaylistEntry::Album(album_id)) =>
                        {
                            push_err(
                                "Album",
                                format!(
                                    "{album_id} lists Playlist {playlist_id} in album.derived.in_playlists, but {playlist_id} does not contain {album_id} in playlist.entries"
                                ),
                            );
                        }
                        _ => {}
                    }
                }
            }
            for (mbid, &album_id) in &self.lookup.album {
                if !self.albums.contains_key(album_id) {
                    push_err(
                        "lookup.album",
                        format!("MBID {mbid} points to missing Album {album_id}"),
                    );
                }
            }
            // 5. Artists -> derived.tracks, derived.track_groups, derived.songs, derived.albums + lookup.artist_*
            for (artist_id, artist) in &self.artists {
                if artist.derived.tracks.is_empty()
                    && artist.derived.track_groups.is_empty()
                    && artist.derived.songs.is_empty()
                    && artist.derived.albums.is_empty()
                {
                    push_err(
                        "Artist",
                        format!(
                            "{artist_id} is orphaned with no tracks, track_groups, songs, or albums"
                        ),
                    );
                }
                if let Some(mbid) = artist.mbid
                    && self.lookup.artist_by_mbid.get(&mbid) != Some(&artist_id)
                {
                    push_err(
                        "Artist",
                        format!(
                            "{artist_id} has missing or mismatched lookup.artist_by_mbid entry for MBID {mbid}"
                        ),
                    );
                }
                for location in &artist.locations {
                    if self.lookup.artist_by_location.get(location) != Some(&artist_id) {
                        push_err(
                            "Artist",
                            format!(
                                "{artist_id} has missing or mismatched lookup.artist_by_location entry for location {location}"
                            ),
                        );
                    }
                }
                for &track_id in &artist.derived.tracks {
                    match self.tracks.get(track_id) {
                        None => push_err(
                            "Artist",
                            format!(
                                "{artist_id} references missing Track {track_id} in artist.derived.tracks"
                            ),
                        ),
                        Some(track) if !track.artists.contains(&artist_id) => push_err(
                            "Artist",
                            format!(
                                "{artist_id} lists Track {track_id} in artist.derived.tracks, but {track_id} does not list {artist_id} in track.artists"
                            ),
                        ),
                        _ => {}
                    }
                }
                for &group_id in &artist.derived.track_groups {
                    match self.track_groups.get(group_id) {
                        None => push_err(
                            "Artist",
                            format!(
                                "{artist_id} references missing TrackGroup {group_id} in artist.derived.track_groups"
                            ),
                        ),
                        Some(group) if !group.artists.contains(&artist_id) => push_err(
                            "Artist",
                            format!(
                                "{artist_id} lists TrackGroup {group_id} in artist.derived.track_groups, but {group_id} does not list {artist_id} in track_group.artists"
                            ),
                        ),
                        _ => {}
                    }
                }
                for &song_id in &artist.derived.songs {
                    match self.songs.get(song_id) {
                        None => push_err(
                            "Artist",
                            format!(
                                "{artist_id} references missing Song {song_id} in artist.derived.songs"
                            ),
                        ),
                        Some(song) if !song.artists.contains(&artist_id) => push_err(
                            "Artist",
                            format!(
                                "{artist_id} lists Song {song_id} in artist.derived.songs, but {song_id} does not list {artist_id} in song.artists"
                            ),
                        ),
                        _ => {}
                    }
                }
                for &album_id in &artist.derived.albums {
                    match self.albums.get(album_id) {
                        None => push_err(
                            "Artist",
                            format!(
                                "{artist_id} references missing Album {album_id} in artist.derived.albums"
                            ),
                        ),
                        Some(album) if !album.artists.contains(&artist_id) => push_err(
                            "Artist",
                            format!(
                                "{artist_id} lists Album {album_id} in artist.derived.albums, but {album_id} does not list {artist_id} in album.artists"
                            ),
                        ),
                        _ => {}
                    }
                }
            }
            for (mbid, &artist_id) in &self.lookup.artist_by_mbid {
                if !self.artists.contains_key(artist_id) {
                    push_err(
                        "lookup.artist_by_mbid",
                        format!("MBID {mbid} points to missing Artist {artist_id}"),
                    );
                }
            }
            for (location, &artist_id) in &self.lookup.artist_by_location {
                if !self.artists.contains_key(artist_id) {
                    push_err(
                        "lookup.artist_by_location",
                        format!("location {location} points to missing Artist {artist_id}"),
                    );
                }
            }
            // 6. Playlists -> entries, derived.in_playlists + lookup.playlist
            for (playlist_id, playlist) in &self.playlists {
                if self.lookup.playlist.get(&playlist.location) != Some(&playlist_id) {
                    push_err(
                        "Playlist",
                        format!(
                            "{playlist_id} has missing or mismatched lookup.playlist entry for location {}",
                            playlist.location
                        ),
                    );
                }
                for entry in &playlist.entries {
                    match *entry {
                        PlaylistEntry::Track(track_id) => match self.tracks.get(track_id) {
                            None => push_err(
                                "Playlist",
                                format!(
                                    "{playlist_id} references missing Track {track_id} in playlist.entries"
                                ),
                            ),
                            Some(track) if !track.derived.in_playlists.contains(&playlist_id) => {
                                push_err(
                                    "Playlist",
                                    format!(
                                        "{playlist_id} contains Track {track_id} in playlist.entries, but {track_id} does not list {playlist_id} in track.derived.in_playlists"
                                    ),
                                );
                            }
                            _ => {}
                        },
                        PlaylistEntry::TrackGroup(group_id) => {
                            match self.track_groups.get(group_id) {
                                None => push_err(
                                    "Playlist",
                                    format!(
                                        "{playlist_id} references missing TrackGroup {group_id} in playlist.entries"
                                    ),
                                ),
                                Some(group)
                                    if !group.derived.in_playlists.contains(&playlist_id) =>
                                {
                                    push_err(
                                        "Playlist",
                                        format!(
                                            "{playlist_id} contains TrackGroup {group_id} in playlist.entries, but {group_id} does not list {playlist_id} in track_group.derived.in_playlists"
                                        ),
                                    );
                                }
                                _ => {}
                            }
                        }
                        PlaylistEntry::Song(song_id) => match self.songs.get(song_id) {
                            None => push_err(
                                "Playlist",
                                format!(
                                    "{playlist_id} references missing Song {song_id} in playlist.entries"
                                ),
                            ),
                            Some(song) if !song.derived.in_playlists.contains(&playlist_id) => {
                                push_err(
                                    "Playlist",
                                    format!(
                                        "{playlist_id} contains Song {song_id} in playlist.entries, but {song_id} does not list {playlist_id} in song.derived.in_playlists"
                                    ),
                                );
                            }
                            _ => {}
                        },
                        PlaylistEntry::Album(album_id) => match self.albums.get(album_id) {
                            None => push_err(
                                "Playlist",
                                format!(
                                    "{playlist_id} references missing Album {album_id} in playlist.entries"
                                ),
                            ),
                            Some(album) if !album.derived.in_playlists.contains(&playlist_id) => {
                                push_err(
                                    "Playlist",
                                    format!(
                                        "{playlist_id} contains Album {album_id} in playlist.entries, but {album_id} does not list {playlist_id} in album.derived.in_playlists"
                                    ),
                                );
                            }
                            _ => {}
                        },
                        PlaylistEntry::Playlist(child_id) => {
                            if child_id == playlist_id {
                                push_err(
                                    "Playlist",
                                    format!("{playlist_id} references itself in playlist.entries"),
                                );
                            }
                            match self.playlists.get(child_id) {
                                None => push_err(
                                    "Playlist",
                                    format!(
                                        "{playlist_id} references missing child Playlist {child_id} in playlist.entries"
                                    ),
                                ),
                                Some(child)
                                    if !child.derived.in_playlists.contains(&playlist_id) =>
                                {
                                    push_err(
                                        "Playlist",
                                        format!(
                                            "{playlist_id} contains Playlist {child_id} in playlist.entries, but {child_id} does not list {playlist_id} in playlist.derived.in_playlists"
                                        ),
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                }
                for &parent_id in &playlist.derived.in_playlists {
                    match self.playlists.get(parent_id) {
                        None => push_err(
                            "Playlist",
                            format!(
                                "{playlist_id} references missing parent Playlist {parent_id} in playlist.derived.in_playlists"
                            ),
                        ),
                        Some(parent)
                            if !parent
                                .entries
                                .contains(&PlaylistEntry::Playlist(playlist_id)) =>
                        {
                            push_err(
                                "Playlist",
                                format!(
                                    "{playlist_id} lists parent Playlist {parent_id} in playlist.derived.in_playlists, but {parent_id} does not contain {playlist_id} in playlist.entries"
                                ),
                            );
                        }
                        _ => {}
                    }
                }
            }
            for (location, &playlist_id) in &self.lookup.playlist {
                if !self.playlists.contains_key(playlist_id) {
                    push_err(
                        "lookup.playlist",
                        format!("location {location} points to missing Playlist {playlist_id}"),
                    );
                }
            }

            errors
        }

        pub fn check_integrity(&self) {
            let errors = self.find_integrity_errors();

            if !errors.is_empty() {
                for err in &errors {
                    error!("Database integrity violation: {err}");
                }
                error!(
                    "DatabaseStore integrity check failed ({} errors)",
                    errors.len()
                );
            }
        }
    }
}
