use std::{num::NonZeroU32, sync::LazyLock, time::Duration};

use governor::{Jitter, Quota, RateLimiter};
use musicbrainz_rs::{
    Fetch, FetchQuery,
    entity::{
        artist::Artist,
        artist_credit::ArtistCredit,
        recording::Recording,
        relations::{Relation, RelationContent},
        release::Release,
    },
};
use thiserror::Error;

use crate::database::{
    jobs::JobContext,
    store::{
        AlbumData, AlbumId, ArtistData, ArtistId, DatabaseStore, DerivedAlbumData,
        DerivedArtistData, DerivedSongData, MissingDatabaseId, MusicBrainzId, SongData, SongId,
    },
};

#[derive(Debug, Error)]
pub enum MusicBrainzFetchError {
    #[error("A query is already pending for {0}")]
    PendingQuery(MusicBrainzId),

    #[error(transparent)]
    DbError(#[from] MissingDatabaseId),

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

fn insert_pending_query(
    ctx: &JobContext,
    mbid: MusicBrainzId,
) -> Result<(), MusicBrainzFetchError> {
    ctx.write_db(|db| db.claim_mbid_query(mbid))
        .map_err(MusicBrainzFetchError::PendingQuery)
}

fn insert_artist(
    db: &mut DatabaseStore,
    mbid: MusicBrainzId,
    mb_artist: Artist,
) -> Result<ArtistId, MissingDatabaseId> {
    let genres = mb_artist
        .genres
        .into_iter()
        .flatten()
        .map(|genre| genre.name)
        .collect();

    let artist = ArtistData {
        mbid: None,
        derived: DerivedArtistData::default(),

        name: Some(mb_artist.name),
        sort_name: Some(mb_artist.sort_name),
        image: None,
        genres,
    };

    db.complete_mbid_query(&mbid);
    let id = db.update_or_insert_artist_by_mbid(mbid, artist)?;

    Ok(id)
}

pub async fn fetch_artist(
    ctx: JobContext,
    mbid: MusicBrainzId,
) -> Result<ArtistId, MusicBrainzFetchError> {
    insert_pending_query(&ctx, mbid)?;
    let mb_artist = fetch_rate_limited(&mbid, Artist::fetch().with_genres()).await?;

    let id = ctx.write_db(|db| insert_artist(db, mbid, mb_artist))?;

    Ok(id)
}

fn get_artists(
    db: &mut DatabaseStore,
    artist_credits: Vec<ArtistCredit>,
) -> Result<(Vec<ArtistId>, Option<String>), MissingDatabaseId> {
    let mut artists = Vec::new();
    let mut artists_text = String::new();

    for credit in artist_credits {
        artists_text.push_str(&credit.artist.name);
        if let Some(join_phrase) = credit.joinphrase {
            artists_text.push_str(&join_phrase);
        }

        let Ok(mbid) = MusicBrainzId::parse_str(&credit.artist.id) else {
            continue;
        };

        let id = db
            .lookup_artist_by_mbid(&mbid)
            .map_or_else(|| insert_artist(db, mbid, credit.artist), Ok)?;

        artists.push(id);
    }

    if artists.is_empty() {
        Ok((artists, None))
    } else {
        Ok((artists, Some(artists_text)))
    }
}

pub async fn fetch_release(
    ctx: JobContext,
    mbid: MusicBrainzId,
) -> Result<AlbumId, MusicBrainzFetchError> {
    insert_pending_query(&ctx, mbid)?;
    let mb_release = fetch_rate_limited(
        &mbid,
        Release::fetch().with_genres().with_artists().with_media(),
    )
    .await?;

    let genres = mb_release
        .genres
        .into_iter()
        .flatten()
        .map(|genre| genre.name)
        .collect();

    let (artists, artists_text) = mb_release
        .artist_credit
        .map(|artist_credits| ctx.write_db(|db| get_artists(db, artist_credits)))
        .transpose()?
        .unwrap_or_default();

    let song_count = mb_release
        .media
        .map(|media| media.iter().map(|medium| medium.track_count).sum());

    let artist = AlbumData {
        mbid: None,
        derived: DerivedAlbumData::default(),

        title: Some(mb_release.title),
        image: None,
        date: mb_release.date.map(Into::into),
        genres,
        track_count: song_count,

        artists,
        artists_text,
    };

    let id = ctx.write_db(|db| {
        db.complete_mbid_query(&mbid);
        db.update_or_insert_album_by_mbid(mbid, artist)
    })?;

    Ok(id)
}

fn insert_song(
    ctx: &JobContext,
    mbid: MusicBrainzId,
    mb_recording: Recording,
) -> Result<SongId, MissingDatabaseId> {
    let genres = mb_recording
        .genres
        .into_iter()
        .flatten()
        .map(|genre| genre.name)
        .collect();

    let (artists, artists_text) = mb_recording
        .artist_credit
        .map(|artist_credits| ctx.write_db(|db| get_artists(db, artist_credits)))
        .transpose()?
        .unwrap_or_default();

    let mix_songs = mb_recording
        .relations
        .map(|relations| get_mix_songs(ctx, relations))
        .transpose()?
        .unwrap_or_default();

    let song = SongData {
        mbid: None,
        derived: DerivedSongData::default(),

        title: Some(mb_recording.title),
        image: None,
        length: mb_recording
            .length
            .map(|ms| Duration::from_millis(u64::from(ms))),
        date: mb_recording.first_release_date.map(Into::into),
        genres,

        artists,
        artists_text,

        mix_songs,
    };

    let id = ctx.write_db(|db| {
        db.complete_mbid_query(&mbid);
        db.update_or_insert_song_by_mbid(mbid, song)
    })?;

    Ok(id)
}

fn get_mix_songs(
    ctx: &JobContext,
    relations: Vec<Relation>,
) -> Result<Vec<SongId>, MissingDatabaseId> {
    relations
        .into_iter()
        .map(|relation| {
            if relation.relation_type != "DJ-mix" {
                return Ok(None);
            }

            let RelationContent::Recording(mix_recording) = relation.content else {
                return Ok(None);
            };

            let Ok(mbid) = MusicBrainzId::parse_str(&mix_recording.id) else {
                return Ok(None);
            };

            let id = ctx
                .read_db(|db| db.lookup_song_by_mbid(&mbid))
                .map_or_else(|| insert_song(ctx, mbid, *mix_recording), Ok)?;

            Ok(Some(id))
        })
        .filter_map(Result::transpose)
        .collect()
}

pub async fn fetch_recording(
    ctx: JobContext,
    mbid: MusicBrainzId,
) -> Result<SongId, MusicBrainzFetchError> {
    insert_pending_query(&ctx, mbid)?;
    let mb_recording = fetch_rate_limited(
        &mbid,
        Recording::fetch()
            .with_genres()
            .with_artists()
            .with_recording_relations(),
    )
    .await?;

    let id = insert_song(&ctx, mbid, mb_recording)?;

    Ok(id)
}
