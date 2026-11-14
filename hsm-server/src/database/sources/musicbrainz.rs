use std::{mem::DropGuard, num::NonZeroU32, sync::LazyLock, time::Duration};

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
) -> Result<DropGuard<(), impl FnOnce(())>, MusicBrainzFetchError> {
    ctx.write_db(|db| db.claim_mbid_query(mbid))
        .map(|()| {
            DropGuard::new((), move |()| {
                ctx.write_db(|db| db.fail_mbid_query(mbid));
            })
        })
        .map_err(MusicBrainzFetchError::PendingQuery)
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
) -> Result<ArtistId, MissingDatabaseId> {
    let artist = ArtistData {
        mbid: Some(mbid),
        locations: Vec::new(),
        derived: DerivedArtistData::default(),

        name: Some(mb_artist.name),
        sort_name: Some(mb_artist.sort_name),
        genres: get_genres(mb_artist.genres),
    };

    let id = db.update_or_insert_artist_by_mbid(mbid, artist)?;
    if is_complete {
        db.complete_mbid_query(mbid);
    }

    Ok(id)
}

pub async fn fetch_artist(
    ctx: JobContext,
    mbid: MusicBrainzId,
) -> Result<(), MusicBrainzFetchError> {
    let guard = insert_pending_query(&ctx, mbid)?;
    let mb_artist = fetch_rate_limited(&mbid, Artist::fetch().with_genres()).await?;

    ctx.write_db(|db| insert_artist(db, mbid, mb_artist, true))?;
    DropGuard::dismiss(guard);

    Ok(())
}

fn get_artists(
    db: &mut DatabaseStore,
    artist_credits: Option<Vec<ArtistCredit>>,
) -> Result<(Vec<ArtistId>, Option<String>), MissingDatabaseId> {
    let Some(artist_credits) = artist_credits else {
        return Ok((Vec::new(), None));
    };

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
            .lookup_artist_by_mbid(mbid)
            .map_or_else(|| insert_artist(db, mbid, credit.artist, false), Ok)?;

        artists.push(id);
    }

    if artists.is_empty() {
        Ok((artists, None))
    } else {
        Ok((artists, Some(artists_text)))
    }
}

fn insert_album(
    db: &mut DatabaseStore,
    mbid: MusicBrainzId,
    mb_release_group: ReleaseGroup,
    is_complete: bool,
) -> Result<AlbumId, MissingDatabaseId> {
    let (artists, artists_text) = get_artists(db, mb_release_group.artist_credit)?;

    let album = AlbumData {
        mbid,
        derived: DerivedAlbumData::default(),

        title: Some(mb_release_group.title),
        date: mb_release_group.first_release_date.map(Into::into),
        genres: get_genres(mb_release_group.genres),

        artists,
        artists_text,
    };

    let id = db.update_or_insert_album(album)?;
    if is_complete {
        db.complete_mbid_query(mbid);
    }

    Ok(id)
}

pub async fn fetch_release_group(
    ctx: JobContext,
    mbid: MusicBrainzId,
) -> Result<(), MusicBrainzFetchError> {
    let guard = insert_pending_query(&ctx, mbid)?;
    let mb_release_group =
        fetch_rate_limited(&mbid, ReleaseGroup::fetch().with_genres().with_artists()).await?;

    ctx.write_db(|db| insert_album(db, mbid, mb_release_group, true))?;
    DropGuard::dismiss(guard);

    Ok(())
}

fn insert_song(
    db: &mut DatabaseStore,
    mbid: MusicBrainzId,
    mb_recording: Recording,
    is_complete: bool,
) -> Result<SongId, MissingDatabaseId> {
    let (artists, artists_text) = get_artists(db, mb_recording.artist_credit)?;

    let mix_songs = get_mix_songs(db, mb_recording.relations)?;

    let song = SongData {
        mbid,
        derived: DerivedSongData::default(),

        title: Some(mb_recording.title),
        length: mb_recording
            .length
            .map(|ms| Duration::from_millis(u64::from(ms))),
        date: mb_recording.first_release_date.map(Into::into),
        genres: get_genres(mb_recording.genres),

        artists,
        artists_text,

        mix_songs,
    };

    let id = db.update_or_insert_song(song)?;
    if is_complete {
        db.complete_mbid_query(mbid);
    }

    Ok(id)
}

fn get_mix_songs(
    db: &mut DatabaseStore,
    relations: Option<Vec<Relation>>,
) -> Result<Vec<SongId>, MissingDatabaseId> {
    let Some(relations) = relations else {
        return Ok(Vec::new());
    };

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

            let id = db
                .lookup_song(&mbid)
                .map_or_else(|| insert_song(db, mbid, *mix_recording, false), Ok)?;

            Ok(Some(id))
        })
        .filter_map(Result::transpose)
        .collect()
}

pub async fn fetch_recording(
    ctx: JobContext,
    mbid: MusicBrainzId,
) -> Result<(), MusicBrainzFetchError> {
    let guard = insert_pending_query(&ctx, mbid)?;
    let mb_recording = fetch_rate_limited(
        &mbid,
        Recording::fetch()
            .with_genres()
            .with_artists()
            .with_recording_relations(),
    )
    .await?;

    ctx.write_db(|db| insert_song(db, mbid, mb_recording, true))?;
    DropGuard::dismiss(guard);

    Ok(())
}
