use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use hsm_ipc::TrackMetadata;
use symphonia::core::{
    codecs::CODEC_TYPE_NULL,
    errors::Error as SymphoniaError,
    formats::FormatOptions,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::{Metadata, MetadataOptions, MetadataRevision, StandardTagKey, Tag, Value},
    probe::{Hint, ProbeResult},
};
use thiserror::Error;

use crate::database::{
    jobs::JobContext,
    store::{AlbumData, ArtistData, MusicBrainzId, ResourceLocation, TrackData},
};

const PLUGIN_ID: &str = "local_files";

#[derive(Debug, Error)]
pub enum LoadTrackError {
    #[error("{0}")]
    IoError(#[source] io::Error),

    #[error("{0}")]
    ProbeFailed(#[source] SymphoniaError),

    #[error("Track has no supported audio codec")]
    CodecNotSupported,
}

/// Use symphonia to probe the track for metadata
/// Uses the default symphonia probe and the path's extension as a `Hint`
fn probe_track_sync(path: &Path) -> Result<ProbeResult, LoadTrackError> {
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(extension);
    }

    // Use the default options for metadata and format readers.
    let meta_opts: MetadataOptions = MetadataOptions::default();
    let fmt_opts: FormatOptions = FormatOptions::default();

    let src = File::open(path).map_err(LoadTrackError::IoError)?;
    let mss = MediaSourceStream::new(Box::new(src), MediaSourceStreamOptions::default());
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(LoadTrackError::ProbeFailed)?;

    Ok(probed)
}

/// Load a `Track` from a specified file path
pub async fn load_file(
    ctx: JobContext,
    path: PathBuf,
) -> Result<(Option<Duration>, Option<Metadata<'static>>), LoadTrackError> {
    smol::unblock(move || {
        let mut probed = probe_track_sync(&path)?;

        let audio_track = probed
            .format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(LoadTrackError::CodecNotSupported)?;

        let codec_params = &audio_track.codec_params;

        let codec_duration: Option<Duration> = codec_params
            .time_base
            .zip(codec_params.n_frames)
            .map(|(base, spans)| base.calc_time(spans).into());

        let metadata = if probed.format.metadata().current().is_some() {
            Some(probed.format.metadata())
        } else {
            probed.metadata.get()
        };

        let mut scan = TrackScan::default();
        scan.path = path;
        scan.duration = codec_duration;

        if let Some(metadata) = metadata.as_ref().and_then(|m| m.current()) {
            for tag in metadata.tags() {
                process_metadata(&mut scan, tag);
            }
        }

        //Ok((total_duration, metadata))
        todo!()
    })
    .await
}

#[derive(Debug, Default)]
pub struct TrackScan {
    path: PathBuf,
    name: Option<String>,
    duration: Option<Duration>,
    date: Option<String>,
    genres: Vec<String>,
    mbid: Option<String>,

    artist: Option<String>,
    artsits: Vec<String>,
    artist_mbids: Vec<String>,

    album: Option<String>,
    track_number: Option<u64>,
    album_mbid: Option<String>,
}

fn split_multi_tag(tag_value: &str) -> impl Iterator<Item = String> {
    tag_value
        .split('\0')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(Into::into)
}

fn process_metadata(scan: &mut TrackScan, tag: &Tag) {
    use StandardTagKey::*;
    match (tag.std_key, &tag.key, &tag.value) {
        (Some(TrackTitle), _, Value::String(title)) => {
            scan.name = Some(title.into());
        }
        (Some(Artist), _, Value::String(artist)) => {
            scan.artist = Some(artist.into());
        }
        (Some(Album), _, Value::String(album)) => {
            scan.album = Some(album.into());
        }
        (Some(Date), _, Value::String(date)) => {
            scan.date = Some(date.into());
        }

        (Some(TrackNumber), _, Value::UnsignedInt(track_number)) => {
            scan.track_number = Some(*track_number);
        }
        (Some(TrackNumber), _, Value::String(track_number))
            if let Ok(track_number) = track_number.parse() =>
        {
            scan.track_number = Some(track_number);
        }

        (Some(Genre), _, Value::String(genres)) => {
            scan.genres.extend(split_multi_tag(genres));
        }

        (Some(MusicBrainzRecordingId), _, Value::String(mbid)) => {
            scan.mbid = Some(mbid.into());
        }
        (Some(MusicBrainzAlbumId), _, Value::String(mbid)) => {
            scan.album_mbid = Some(mbid.into());
        }
        (Some(MusicBrainzArtistId), _, Value::String(mbids)) => {
            scan.artist_mbids.extend(split_multi_tag(mbids));
        }
        _ => (),
    }
}

//pub async fn scan_file_path(ctx: JobContext, path: PathBuf) {}
