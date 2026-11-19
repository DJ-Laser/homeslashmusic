use std::{cmp, collections::HashSet};

use nucleo_matcher::{Matcher, Utf32String, pattern::Pattern};

use crate::database::{
    jobs::JobContext,
    store::{AlbumId, ArtistId, DatabaseStore, MissingDatabaseId, SongId, TrackGroupId, TrackId},
};

pub enum SearchableEntityId {
    Track(TrackId),
    TrackGroup(TrackGroupId),
    Song(SongId),
    Album(AlbumId),
    Artist(ArtistId),
}

impl DatabaseStore {
    pub fn get_artists_text(
        &self,
        artists_text: Option<&String>,
        artists: &Vec<ArtistId>,
    ) -> Result<Option<String>, MissingDatabaseId> {
        if let Some(artists_text) = artists_text {
            return Ok(Some(artists_text.clone()));
        }

        if artists.is_empty() {
            return Ok(None);
        }

        let mut artists_text = String::new();

        for artist_id in artists {
            let artist = self.get_artist(*artist_id)?;
            if !artists_text.is_empty() {
                artists_text.push_str(" & ");
            }

            artists_text.push_str(&artist.name);
        }

        Ok(Some(artists_text))
    }

    fn search_string(
        &self,
        title: &String,
        artists_text: Option<&String>,
        artists: &Vec<ArtistId>,
    ) -> Result<Utf32String, MissingDatabaseId> {
        let search_string = match self.get_artists_text(artists_text, artists)? {
            None => Utf32String::from(title.as_str()),
            Some(artist) => Utf32String::from(format!("{title} - {artist}")),
        };

        Ok(search_string)
    }

    fn get_track_or_song_ids(
        &self,
    ) -> Result<Vec<(SearchableEntityId, Utf32String)>, MissingDatabaseId> {
        let mut songs = HashSet::new();
        let mut items = Vec::new();

        for (track_id, track) in &self.tracks {
            if let Some(song_id) = track.song {
                let song = self.get_song(song_id)?;

                if songs.insert(song_id) {
                    items.push((
                        SearchableEntityId::Song(song_id),
                        self.search_string(&song.title, song.artists_text.as_ref(), &song.artists)?,
                    ));
                }
            } else {
                items.push((
                    SearchableEntityId::Track(track_id),
                    self.search_string(&track.title, track.artists_text.as_ref(), &track.artists)?,
                ));
            }
        }

        Ok(items)
    }

    fn get_track_group_or_album_ids(
        &self,
    ) -> Result<Vec<(SearchableEntityId, Utf32String)>, MissingDatabaseId> {
        let mut albums = HashSet::new();
        let mut items = Vec::new();

        for (track_group_id, track_group) in &self.track_groups {
            if let Some(album_id) = track_group.album {
                let album = self.get_album(album_id)?;

                if albums.insert(album_id) {
                    items.push((
                        SearchableEntityId::Album(album_id),
                        self.search_string(
                            &album.title,
                            album.artists_text.as_ref(),
                            &album.artists,
                        )?,
                    ));
                }
            } else {
                items.push((
                    SearchableEntityId::TrackGroup(track_group_id),
                    self.search_string(
                        &track_group.title,
                        track_group.artists_text.as_ref(),
                        &track_group.artists,
                    )?,
                ));
            }
        }

        Ok(items)
    }

    /// Get all searchable entities and their search string
    pub fn get_searchable_entities(
        &self,
    ) -> Result<Vec<(SearchableEntityId, Utf32String)>, MissingDatabaseId> {
        let mut items: Vec<_> = self
            .artists
            .iter()
            .map(|(artist_id, artist)| {
                (
                    SearchableEntityId::Artist(artist_id),
                    Utf32String::from(artist.name.as_str()),
                )
            })
            .collect();
        items.extend(self.get_track_or_song_ids()?);
        items.extend(self.get_track_group_or_album_ids()?);

        Ok(items)
    }
}

pub async fn run_search(
    ctx: JobContext,
    search_query: String,
) -> Result<Vec<(SearchableEntityId, u32)>, MissingDatabaseId> {
    use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization};

    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
    let pattern = Pattern::new(
        &search_query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let items = ctx.read_db(DatabaseStore::get_searchable_entities)?;

    let mut scores: Vec<_> = items
        .into_iter()
        .map(|(item, haystack)| (item, pattern.score(haystack.slice_u32(..), &mut matcher)))
        .filter_map(|(item, score)| score.map(|score| (item, score)))
        .collect();

    scores.sort_by_key(|(_, score)| cmp::Reverse(*score));

    Ok(scores)
}
