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
    store::{AlbumData, AlbumId, ArtistData, ArtistId, MusicBrainzId, SongData, SongId},
};

const PLUGIN_ID: &str = "local_files";

#[derive(Debug, Error)]
pub enum LoadFileError {
    #[error("{0}")]
    IoError(#[from] io::Error),

    #[error("{0}")]
    ParseFailed(#[from] lofty::error::FileParseError),

    #[error("song missing MusicBrainz id")]
    MissingMbid,
}

/// Load a `song` from a specified file path
pub async fn parse_file_tags(path: PathBuf) -> Result<(Duration, Option<Tag>), LoadFileError> {
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
    let mbid = MusicBrainzId::parse_str(tag.get_string(ItemKey::MusicBrainzReleaseId)?).ok()?;

    if let Some(id) = ctx.read_db(|db| db.lookup_album_by_mbid(&mbid)) {
        return Some(id);
    }

    let name = tag.album().map(Into::into);
    let artists = get_artists(ctx, tag, true);
    let artists_text = tag.get_string(ItemKey::AlbumArtist).map(Into::into);
    let genres = get_genres(tag);
    let date = get_date(tag);
    let song_count = tag.track_total();

    let album = ctx.write_db(|db| {
        db.insert_album(AlbumData {
            mbid: Some(mbid),
            title: name,
            artists,
            artists_text,
            genres,
            date,
            track_count: song_count,
            ..Default::default()
        })
    });

    ctx.spawn_detatched(musicbrainz::fetch_release(ctx.clone(), mbid));

    Some(album)
}

fn parse_song_length(input: &str) -> Option<Duration> {
    let mut parts = input.trim().rsplit(':');

    let secs: u64 = parts.next()?.parse().ok()?;

    let mins: u64 = parts.next()?.parse().ok()?;

    let hours: u64 = match parts.next() {
        Some(h) => h.parse().ok()?,
        None => 0,
    };

    // Don't accept input with four or more colon separated values
    if parts.next().is_some() {
        return None;
    }

    let total_secs = hours * 3600 + mins * 60 + secs;
    Some(Duration::from_secs(total_secs))
}

fn get_song(ctx: &JobContext, tag: &Tag) -> Option<SongId> {
    //TODO: Support not having an mbid
    let mbid = MusicBrainzId::parse_str(tag.get_string(ItemKey::MusicBrainzRecordingId)?).ok()?;

    if let Some(id) = ctx.read_db(|db| db.lookup_song_by_mbid(&mbid)) {
        return Some(id);
    }

    let name = tag.title().map(Into::into);
    let artists = get_artists(ctx, tag, false);
    let artists_text = tag.get_string(ItemKey::TrackArtist).map(Into::into);
    let genres = get_genres(tag);
    let date = get_date(tag);
    let length = tag.get_string(ItemKey::Length).and_then(parse_song_length);

    let song = ctx.write_db(|db| {
        db.insert_song(SongData {
            mbid: Some(mbid),
            title: name,
            artists,
            artists_text,
            genres,
            date,
            length,
            ..Default::default()
        })
    });

    ctx.spawn_detatched(musicbrainz::fetch_recording(ctx.clone(), mbid));

    Some(song)
}

async fn load_song_file(
    ctx: JobContext,
    path: PathBuf,
) -> Result<(Option<SongId>, Option<AlbumId>), LoadFileError> {
    let (_duration, Some(ref tag)) = parse_file_tags(path).await? else {
        return Err(LoadFileError::MissingMbid);
    };

    let song = get_song(&ctx, tag);
    let album = get_album(&ctx, tag);

    Ok((song, album))
}

async fn search_directory(ctx: JobContext, path: PathBuf) -> Vec<(PathBuf, LoadFileError)> {
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
) -> impl Future<Output = Vec<(PathBuf, LoadFileError)>> + Send {
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
            match load_song_file(ctx, path.clone()).await {
                Ok(_) => Vec::new(),
                Err(error) => vec![(path, error)],
            }
        }
    })
}

pub async fn load_all_from_path(ctx: JobContext, path: PathBuf) {
    search_file_or_directory(ctx, path).await;
}
