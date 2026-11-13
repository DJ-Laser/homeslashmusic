use crate::database::{
    jobs::JobContext,
    store::{ArtistId, MusicBrainzId},
};

pub async fn fetch_artist(ctx: JobContext, mbid: MusicBrainzId) -> Option<ArtistId> {
    todo!()
}

pub async fn fetch_release(ctx: JobContext, mbid: MusicBrainzId) -> Option<ArtistId> {
    todo!()
}

pub async fn fetch_recording(ctx: JobContext, mbid: MusicBrainzId) -> Option<ArtistId> {
    todo!()
}
