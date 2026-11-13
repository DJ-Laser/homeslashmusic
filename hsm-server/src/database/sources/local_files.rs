use std::{io, iter, path::PathBuf, time::Duration};

use lofty::{
    file::{AudioFile, TaggedFileExt},
    tag::{Accessor, ItemKey, Tag},
};
use smol::stream::StreamExt as _;
use thiserror::Error;

use crate::database::{
    jobs::JobContext,
    sources::musicbrainz,
    store::{AlbumData, AlbumId, ArtistData, ArtistId, MusicBrainzId, TrackData, TrackId},
};

const PLUGIN_ID: &str = "local_files";

#[derive(Debug, Error)]
pub enum LoadTrackError {
    #[error("{0}")]
    IoError(#[from] io::Error),

    #[error("{0}")]
    ParseFailed(#[from] lofty::error::FileParseError),

    #[error("Track missing MusicBrainz id")]
    MissingMbid,
}

/// Load a `Track` from a specified file path
pub async fn parse_file_tags(path: PathBuf) -> Result<(Duration, Option<Tag>), LoadTrackError> {
    smol::unblock(move || {
        let mut tagged_file = lofty::read_from_path(path)?;

        let tag = tagged_file
            .remove(tagged_file.primary_tag_type())
            .or_else(|| {
                let tag_type = tagged_file.first_tag()?.tag_type();
                tagged_file.remove(tag_type)
            });

        Ok((tagged_file.properties().duration(), tag))
    })
    .await
}

fn get_artists(ctx: &JobContext, tag: &Tag, album: bool) -> Vec<ArtistId> {
    let (artist_key, multi_artist_key, multi_mbid_key) = if album {
        (
            ItemKey::AlbumArtist,
            ItemKey::AlbumArtists,
            ItemKey::MusicBrainzReleaseArtistId,
        )
    } else {
        (
            ItemKey::TrackArtist,
            ItemKey::TrackArtists,
            ItemKey::MusicBrainzArtistId,
        )
    };

    let mut artists = Vec::new();

    let mbids: Vec<_> = tag.get_strings(multi_mbid_key).collect();
    let names: Vec<_> = tag.get_strings(multi_artist_key).collect();

    ctx.write_db(|db| {
        let mut find_artist = |mbid, name: Option<&str>| {
            let Ok(mbid) = MusicBrainzId::parse_str(mbid) else {
                return;
            };

            if let Some(id) = db.lookup_artist_by_mbid(&mbid) {
                artists.push(id);
                return;
            }

            let id = db.insert_artist(ArtistData {
                mbid: Some(mbid),
                name: name.map(Into::into),
                ..Default::default()
            });

            artists.push(id);
            ctx.spawn_detatched(musicbrainz::fetch_artist(ctx.clone(), mbid));
        };

        if names.is_empty() && mbids.len() == 1 {
            // Fall back to single-value tag if there is only one mbid
            let mbid = mbids.first().expect("verified len == 1");
            let name = tag.get_string(artist_key);

            find_artist(*mbid, name);
        } else if !mbids.is_empty() {
            // For each mbid insert an artist, which may or may not have a name
            // If names.len() > mbids.len() extra names will be discarded
            for (mbid, name) in mbids
                .into_iter()
                .zip(names.into_iter().map(Some).chain(iter::repeat(None)))
            {
                find_artist(mbid, name);
            }
        }
    });

    artists
}

fn get_genres(tag: &Tag) -> Vec<String> {
    tag.get_strings(ItemKey::Genre).map(Into::into).collect()
}

fn get_date(tag: &Tag) -> Option<String> {
    tag.get_string(ItemKey::RecordingDate)
        .or_else(|| tag.get_string(ItemKey::Year))
        .map(Into::into)
}

fn get_album(ctx: &JobContext, tag: &Tag) -> Option<AlbumId> {
    //TODO: Support not having an mbid
    let mbid = tag.get_string(ItemKey::MusicBrainzReleaseId)?;

    let Ok(mbid) = MusicBrainzId::parse_str(mbid) else {
        return None;
    };

    if let Some(id) = ctx.read_db(|db| db.lookup_album_by_mbid(&mbid)) {
        return Some(id);
    }

    let name = tag.album().map(Into::into);
    let artists = get_artists(ctx, tag, true);
    let artists_text = tag.get_string(ItemKey::AlbumArtist).map(Into::into);
    let genres = get_genres(tag);
    let date = get_date(tag);
    let track_count = tag.track_total();

    let album = ctx.write_db(|db| {
        db.insert_album(AlbumData {
            mbid: Some(mbid),
            name,
            artists,
            artists_text,
            genres,
            date,
            track_count,
            ..Default::default()
        })
    });

    ctx.spawn_detatched(musicbrainz::fetch_release(ctx.clone(), mbid));

    Some(album)
}

async fn load_track_file(ctx: JobContext, path: PathBuf) -> Result<TrackId, LoadTrackError> {
    let (duration, Some(ref tag)) = parse_file_tags(path).await? else {
        return Err(LoadTrackError::MissingMbid);
    };

    //TODO: Support not having an mbid
    let mbid = tag
        .get_string(ItemKey::MusicBrainzRecordingId)
        .ok_or(LoadTrackError::MissingMbid)?;

    let Ok(mbid) = MusicBrainzId::parse_str(mbid) else {
        return Err(LoadTrackError::MissingMbid);
    };

    if let Some(id) = ctx.read_db(|db| db.lookup_track_by_mbid(&mbid)) {
        return Ok(id);
    }

    let name = tag.title().map(Into::into);
    let artists = get_artists(&ctx, tag, false);
    let artists_text = tag.get_string(ItemKey::TrackArtist).map(Into::into);
    let album = get_album(&ctx, tag);
    let track_number = tag.track_total();
    let genres = get_genres(tag);
    let date = get_date(tag);

    let track = ctx.write_db(|db| {
        db.insert_track(TrackData {
            mbid: Some(mbid),
            name,
            artists,
            artists_text,
            album,
            track_number,
            genres,
            date,
            length: Some(duration),
            ..Default::default()
        })
    });

    ctx.spawn_detatched(musicbrainz::fetch_recording(ctx.clone(), mbid));

    Ok(track)
}

async fn search_directory(ctx: JobContext, path: PathBuf) -> Vec<(PathBuf, LoadTrackError)> {
    let mut entries = match smol::fs::read_dir(&path).await {
        Ok(files) => files,
        Err(error) => {
            return vec![(path, error.into())];
        }
    };

    let mut errors = Vec::new();
    let mut futures = Vec::new();

    while let Some(entry) = entries.next().await {
        let entry_path = match entry {
            Ok(entry) => entry.path(),
            Err(error) => {
                errors.push((path.clone(), error.into()));
                continue;
            }
        };

        futures.push(ctx.spawn(search_file_or_directory(ctx.clone(), entry_path)));
    }

    for fut in futures {
        errors.extend(fut.await);
    }

    errors
}

fn search_file_or_directory(
    ctx: JobContext,
    path: PathBuf,
) -> impl Future<Output = Vec<(PathBuf, LoadTrackError)>> + Send {
    Box::pin(async move {
        let metadata = match smol::fs::metadata(&path).await {
            Ok(metadata) => metadata,
            Err(error) => {
                return vec![(path, error.into())];
            }
        };

        if metadata.is_dir() {
            search_directory(ctx, path).await
        } else {
            match load_track_file(ctx, path.clone()).await {
                Ok(_) => Vec::new(),
                Err(error) => vec![(path, error)],
            }
        }
    })
}

pub async fn load_tracks_from_path(ctx: JobContext, path: PathBuf) {
    search_file_or_directory(ctx, path).await;
}
