use std::{collections::HashMap, num::NonZeroU32, sync::LazyLock, time::Duration};

use governor::{Jitter, Quota, RateLimiter};
use musicbrainz_rs::{
    Fetch, FetchQuery,
    entity::{
        artist::Artist,
        artist_credit::ArtistCredit,
        genre::Genre,
        recording::Recording,
        relations::{Relation, RelationContent},
        release_group::ReleaseGroup,
    },
};
use thiserror::Error;

use crate::database::{
    DatabaseHandle,
    store::{
        AlbumData, AlbumId, ArtistData, ArtistId, DatabaseStore, DerivedAlbumData,
        DerivedArtistData, DerivedSongData, MusicBrainzId, SongData, SongId, SongRelationship,
    },
};

#[derive(Debug, Error)]
pub enum MusicBrainzFetchError {
    #[error(transparent)]
    ApiError(Box<musicbrainz_rs::ApiEndpointError>),
}

impl From<musicbrainz_rs::ApiEndpointError> for MusicBrainzFetchError {
    fn from(value: musicbrainz_rs::ApiEndpointError) -> Self {
        Self::ApiError(Box::new(value))
    }
}

type GlobalLimiter = RateLimiter<
    governor::state::NotKeyed,
    governor::state::InMemoryState,
    governor::clock::DefaultClock,
    governor::middleware::NoOpMiddleware,
>;

async fn fetch_rate_limited<T>(
    mbid: &MusicBrainzId,
    query: &mut FetchQuery<T>,
) -> Result<T, MusicBrainzFetchError>
where
    T: Clone + serde::de::DeserializeOwned + Sync,
    T: musicbrainz_rs::Fetch + musicbrainz_rs::APIPath,
{
    static MB_LIMITER: LazyLock<GlobalLimiter> = LazyLock::new(|| {
        let quota = Quota::per_second(NonZeroU32::new(1).unwrap());
        RateLimiter::direct(quota)
    });

    MB_LIMITER
        .until_ready_with_jitter(Jitter::up_to(Duration::from_millis(250)))
        .await;

    query
        .id(&mbid.to_string())
        .execute_async()
        .await
        .map_err(Into::into)
}

fn get_genres(genres: Option<Vec<Genre>>) -> Vec<String> {
    genres
        .into_iter()
        .flatten()
        .map(|genre| genre.name)
        .collect()
}

fn insert_artist(
    db: &mut DatabaseStore,
    mbid: MusicBrainzId,
    mb_artist: Artist,
    is_complete: bool,
) -> ArtistId {
    let artist = ArtistData {
        mbid: Some(mbid),
        locations: Vec::new(),
        derived: DerivedArtistData::default(),

        name: mb_artist.name,
        sort_name: Some(mb_artist.sort_name),
        genres: get_genres(mb_artist.genres),
    };

    let id = db.update_or_insert_artist_by_mbid(mbid, artist);
    if is_complete {
        db.complete_mbid_query(mbid);
    }

    id
}

pub async fn fetch_artist(
    handle: &DatabaseHandle,
    mbid: MusicBrainzId,
) -> Result<(), MusicBrainzFetchError> {
    let mb_artist = fetch_rate_limited(&mbid, Artist::fetch().with_genres()).await?;

    handle.write_db(|db| insert_artist(db, mbid, mb_artist, true));

    Ok(())
}

fn get_artists(
    db: &mut DatabaseStore,
    artist_credits: Option<Vec<ArtistCredit>>,
) -> (Vec<ArtistId>, Option<String>) {
    let Some(artist_credits) = artist_credits else {
        return (Vec::new(), None);
    };

    let mut artists = Vec::new();
    let mut artists_text = String::new();

    for credit in artist_credits {
        artists_text.push_str(&credit.name);
        if let Some(join_phrase) = credit.joinphrase {
            artists_text.push_str(&join_phrase);
        }

        let Ok(mbid) = MusicBrainzId::parse_str(&credit.artist.id) else {
            continue;
        };

        let id = db
            .lookup_artist_by_mbid(mbid)
            .unwrap_or_else(|| insert_artist(db, mbid, credit.artist, false));

        artists.push(id);
    }

    if artists.is_empty() {
        (artists, None)
    } else {
        (artists, Some(artists_text))
    }
}

fn insert_album(
    db: &mut DatabaseStore,
    mbid: MusicBrainzId,
    mb_release_group: ReleaseGroup,
    is_complete: bool,
) -> AlbumId {
    let (artists, artists_text) = get_artists(db, mb_release_group.artist_credit);

    let album = AlbumData {
        mbid,
        derived: DerivedAlbumData::default(),

        title: mb_release_group.title,
        date: mb_release_group.first_release_date.map(Into::into),
        genres: get_genres(mb_release_group.genres),

        artists,
        artists_text,
    };

    let id = db.update_or_insert_album(album);
    if is_complete {
        db.complete_mbid_query(mbid);
    }

    id
}

pub async fn fetch_release_group(
    handle: &DatabaseHandle,
    mbid: MusicBrainzId,
) -> Result<(), MusicBrainzFetchError> {
    let mb_release_group =
        fetch_rate_limited(&mbid, ReleaseGroup::fetch().with_genres().with_artists()).await?;

    handle.write_db(|db| insert_album(db, mbid, mb_release_group, true));

    Ok(())
}

fn insert_song(
    db: &mut DatabaseStore,
    mbid: MusicBrainzId,
    mb_recording: Recording,
    is_complete: bool,
) -> SongId {
    let (artists, artists_text) = get_artists(db, mb_recording.artist_credit);

    let relationships = if is_complete {
        get_song_relationships(db, mb_recording.relations)
    } else {
        HashMap::new()
    };

    let song = SongData {
        mbid,
        derived: DerivedSongData::default(),

        title: mb_recording.title,
        length: mb_recording
            .length
            .map(|ms| Duration::from_millis(u64::from(ms))),
        date: mb_recording.first_release_date.map(Into::into),
        genres: get_genres(mb_recording.genres),

        artists,
        artists_text,

        relationships,
    };

    let id = db.update_or_insert_song(song);
    if is_complete {
        db.complete_mbid_query(mbid);
    }

    id
}

fn get_song_relationships(
    db: &mut DatabaseStore,
    relations: Option<Vec<Relation>>,
) -> HashMap<SongRelationship, Vec<SongId>> {
    let Some(mut relations) = relations else {
        return HashMap::new();
    };

    // Preserve ordered track lists, and put unordered relationships at the end
    relations.sort_by_key(|rel| rel.ordering_key.unwrap_or(u64::MAX));

    let mut relationships: HashMap<SongRelationship, Vec<SongId>> = HashMap::new();

    for relation in relations {
        // Forward means that this song is the remix/compilation
        if relation.direction != "forward" {
            continue;
        }

        let kind = match relation.relation_type.as_str() {
            "remix" => SongRelationship::Remix,
            "mashes up" => SongRelationship::Mashup,
            "DJ-mix" => SongRelationship::DjMix,
            "compilation" => SongRelationship::Compilation,
            _ => continue,
        };

        let RelationContent::Recording(target_recording) = relation.content else {
            continue;
        };

        let Ok(mbid) = MusicBrainzId::parse_str(&target_recording.id) else {
            continue;
        };

        let target_id = db
            .lookup_song(&mbid)
            .unwrap_or_else(|| insert_song(db, mbid, *target_recording, false));

        let list = relationships.entry(kind).or_default();
        if !list.contains(&target_id) {
            list.push(target_id);
        }
    }

    relationships
}

pub async fn fetch_recording(
    handle: &DatabaseHandle,
    mbid: MusicBrainzId,
) -> Result<(), MusicBrainzFetchError> {
    let mb_recording = fetch_rate_limited(
        &mbid,
        Recording::fetch()
            .with_genres()
            .with_artists()
            .with_recording_relations(),
    )
    .await?;

    handle.write_db(|db| insert_song(db, mbid, mb_recording, true));

    Ok(())
}
