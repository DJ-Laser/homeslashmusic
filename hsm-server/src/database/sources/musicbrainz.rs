use std::{num::NonZeroU32, sync::LazyLock, time::Duration};

use governor::{Jitter, Quota, RateLimiter};
use musicbrainz_rs::entity::{artist::Artist, recording::Recording, release::Release};

use crate::database::{
    jobs::JobContext,
    store::{ArtistId, MusicBrainzId},
};

type GlobalLimiter = RateLimiter<
    governor::state::NotKeyed,
    governor::state::InMemoryState,
    governor::clock::DefaultClock,
    governor::middleware::NoOpMiddleware,
>;

#[expect(clippy::result_large_err)]
async fn fetch_rate_limited<T>(mbid: &MusicBrainzId) -> Result<T, musicbrainz_rs::ApiEndpointError>
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

    T::fetch().id(&mbid.to_string()).execute_async().await
}

pub async fn fetch_artist(ctx: JobContext, mbid: MusicBrainzId) -> Option<ArtistId> {
    let response = match fetch_rate_limited::<Artist>(&mbid).await {
        Ok(response) => response,
        Err(err) => {
            eprintln!("{err:?}");
            return None;
        }
    };

    println!("{response:?}");

    None
}

pub async fn fetch_release(ctx: JobContext, mbid: MusicBrainzId) -> Option<ArtistId> {
    let response = match fetch_rate_limited::<Release>(&mbid).await {
        Ok(response) => response,
        Err(err) => {
            eprintln!("{err:?}");
            return None;
        }
    };

    println!("{response:?}");

    None
}

pub async fn fetch_recording(ctx: JobContext, mbid: MusicBrainzId) -> Option<ArtistId> {
    let response = match fetch_rate_limited::<Recording>(&mbid).await {
        Ok(response) => response,
        Err(err) => {
            eprintln!("{err:?}");
            return None;
        }
    };

    println!("{response:?}");

    None
}
