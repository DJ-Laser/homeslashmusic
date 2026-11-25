use std::{collections::HashMap, io, iter, ops::ControlFlow, path::PathBuf, time::Duration};

use lofty::{
    file::{AudioFile, TaggedFileExt},
    tag::{Accessor, ItemKey, Tag},
};
use smol::stream::StreamExt as _;
use thiserror::Error;

use crate::database::{
    sources::{ArtistLocation, TrackGroupLocation, TrackLocation, UpdateContext},
    store::{
        AlbumData, AlbumId, ArtistData, ArtistId, DerivedAlbumData, DerivedArtistData,
        DerivedSongData, DerivedTrackData, DerivedTrackGroupData, MbidQueryType, MissingDatabaseId,
        MusicBrainzId, ReplayGain, SongData, SongId, TrackData, TrackGroupData, TrackGroupId,
        TrackId,
    },
};

#[derive(Debug, Error)]
pub enum LoadFileError {
    #[error("{0}")]
    IoError(#[from] io::Error),

    #[error("{0}")]
    ParseFailed(#[from] lofty::error::FileParseError),

    #[error("{0}")]
    Database(#[from] MissingDatabaseId),

    #[error("File had no metadata to parse")]
    MissingTags,
}

/// Load a `song` from a specified file path
async fn parse_file_tags(path: PathBuf) -> Result<(Duration, Option<Tag>), LoadFileError> {
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

fn get_artists(ctx: &UpdateContext, tag: &Tag, album: bool) -> Vec<ArtistId> {
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

            let location = name.map(|name| ArtistLocation::LocalTags { name: name.into() });
            let id = if let Some(location) = location {
                db.add_location_to_artist_by_mbid(mbid, location)
                    .map_err(Some)
            } else {
                db.lookup_artist_by_mbid(mbid).ok_or(location)
            };

            let location = match id {
                Ok(id) => {
                    artists.push(id);
                    return;
                }
                Err(location) => location,
            };

            db.set_mbid_incomplete(mbid, MbidQueryType::Artist);
            let id = db.insert_artist(ArtistData {
                mbid: Some(mbid),
                locations: location.into_iter().collect(),
                derived: DerivedArtistData::default(),

                name: name.unwrap_or("Unknown Artist").into(),
                sort_name: None,
                genres: Vec::new(),
            });

            artists.push(id);
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

fn get_song(ctx: &UpdateContext, tag: &Tag) -> Option<SongId> {
    let mbid = MusicBrainzId::parse_str(tag.get_string(ItemKey::MusicBrainzRecordingId)?).ok()?;

    if let Some(id) = ctx.read_db(|db| db.lookup_song(&mbid)) {
        return Some(id);
    }

    let title = tag.title().map(Into::into);
    let artists = get_artists(ctx, tag, false);
    let artists_text = tag.get_string(ItemKey::TrackArtist).map(Into::into);
    let genres = get_genres(tag);
    let date = get_date(tag);
    let length = tag.get_string(ItemKey::Length).and_then(parse_song_length);

    let song = ctx.write_db(|db| {
        db.set_mbid_incomplete(mbid, MbidQueryType::Recording);
        db.insert_song(SongData {
            mbid,
            derived: DerivedSongData::default(),

            title: title.unwrap_or_else(|| "Unknown Album".into()),
            length,
            date,
            genres,

            artists,
            artists_text,

            relationships: HashMap::new(),
        })
    });

    Some(song)
}

fn get_album(ctx: &UpdateContext, tag: &Tag) -> Option<AlbumId> {
    let mbid =
        MusicBrainzId::parse_str(tag.get_string(ItemKey::MusicBrainzReleaseGroupId)?).ok()?;

    if let Some(id) = ctx.read_db(|db| db.lookup_album(mbid)) {
        return Some(id);
    }

    let title = tag.album().map(Into::into);
    let artists = get_artists(ctx, tag, true);
    let artists_text = tag.get_string(ItemKey::AlbumArtist).map(Into::into);
    let genres = get_genres(tag);
    let date = get_date(tag);

    let album = ctx.write_db(|db| {
        db.set_mbid_incomplete(mbid, MbidQueryType::ReleaseGroup);
        db.insert_album(AlbumData {
            mbid,
            derived: DerivedAlbumData::default(),

            title: title.unwrap_or_else(|| "Unknown Album".into()),
            artists,
            artists_text,
            genres,
            date,
        })
    });

    Some(album)
}

async fn get_track_group(ctx: &UpdateContext, tag: &Tag) -> Option<TrackGroupId> {
    let title: String = tag.album().map(Into::into)?;
    let location_artist: String = tag
        .get_string(ItemKey::AlbumArtist)
        .or(tag.get_string(ItemKey::TrackArtist))
        .map(Into::into)?;

    let location = TrackGroupLocation::LocalTags {
        provider: ctx.provider().clone(),
        title: title.clone(),
        artist: location_artist,
    };

    let guard = match ctx.mark_track_group(location).await {
        ControlFlow::Continue(location) => location,
        ControlFlow::Break(id) => return Some(id),
    };

    let album = get_album(ctx, tag);
    let artists = get_artists(ctx, tag, true);
    let artists_text = tag.get_string(ItemKey::AlbumArtist).map(Into::into);
    let track_count = tag.track_total();

    let album = ctx.write_db(|db| {
        db.insert_track_group(TrackGroupData {
            provider: ctx.provider().clone(),
            location: guard.location,
            derived: DerivedTrackGroupData::default(),

            album,

            title,
            track_count,

            artists,
            artists_text,
        })
    });

    Some(album)
}

fn get_replay_gain(tag: &Tag) -> ReplayGain {
    fn parse_gain_db(val: &str) -> Option<f32> {
        let val = val.trim();

        // Common cases for "db" suffix
        let val = val
            .strip_suffix("db")
            .or_else(|| val.strip_suffix("DB"))
            .or_else(|| val.strip_suffix("dB"))
            .unwrap_or(val);

        val.trim().parse::<f32>().ok().filter(|v| v.is_finite())
    }

    fn parse_peak(val: &str) -> Option<f32> {
        val.trim()
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite() && *v > 0.0)
    }

    let track_gain = tag
        .get_string(ItemKey::ReplayGainTrackGain)
        .and_then(parse_gain_db);

    let track_peak = tag
        .get_string(ItemKey::ReplayGainTrackPeak)
        .and_then(parse_peak);

    let album_gain = tag
        .get_string(ItemKey::ReplayGainAlbumGain)
        .and_then(parse_gain_db);

    let album_peak = tag
        .get_string(ItemKey::ReplayGainAlbumPeak)
        .and_then(parse_peak);

    ReplayGain {
        track_gain,
        track_peak,
        album_gain,
        album_peak,
    }
}

async fn load_file(ctx: UpdateContext, path: PathBuf) -> Result<TrackId, LoadFileError> {
    let location = TrackLocation::LocalFile(path.clone());

    let guard = match ctx.mark_track(location).await {
        ControlFlow::Continue(location) => location,
        ControlFlow::Break(id) => return Ok(id),
    };

    let (length, Some(ref tag)) = parse_file_tags(path.clone()).await? else {
        return Err(LoadFileError::MissingTags);
    };

    let song = get_song(&ctx, tag);
    let track_group = get_track_group(&ctx, tag).await;

    let title = tag.title().map(Into::into);
    let track_number = tag.track();
    let artists = get_artists(&ctx, tag, false);
    let artists_text = tag.get_string(ItemKey::AlbumArtist).map(Into::into);

    let track = ctx.write_db(|db| {
        db.update_or_insert_track(TrackData {
            provider: ctx.provider().clone(),
            location: guard.location,
            derived: DerivedTrackData::default(),

            song,
            track_group,

            title: title.unwrap_or_else(|| {
                path.file_stem()
                    .expect("load_song_file needs a file path")
                    .to_string_lossy()
                    .into_owned()
            }),
            length,
            track_number,

            artists,
            artists_text,

            replay_gain: get_replay_gain(tag),
        })
    })?;

    Ok(track)
}

async fn search_directory(ctx: UpdateContext, path: PathBuf) -> Vec<(PathBuf, LoadFileError)> {
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
    ctx: UpdateContext,
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
            match load_file(ctx, path.clone()).await {
                Ok(_) => Vec::new(),
                Err(error) => vec![(path, error)],
            }
        }
    })
}

pub async fn load_all_from_path(ctx: UpdateContext, path: PathBuf) {
    search_file_or_directory(ctx, path).await;
}
