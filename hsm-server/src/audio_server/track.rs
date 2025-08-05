use std::{
  fs::File as SyncFile,
  io,
  path::{Path, PathBuf},
  pin::pin,
};

use async_fn_stream::fn_stream;
use hsm_ipc::{AudioSpec, Track, TrackMetadata};
use smol::{
  fs,
  stream::{Stream, StreamExt},
};
use symphonia::core::{
  audio::SignalSpec,
  codecs::{CODEC_TYPE_NULL, Decoder, DecoderOptions},
  errors::Error as SymphoniaError,
  formats::{FormatOptions, FormatReader},
  io::MediaSourceStream,
  meta::{Metadata, MetadataOptions, StandardTagKey, Tag, Value},
  probe::{Hint, ProbeResult},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadTrackError {
  #[error("{0}")]
  CannonicalizeFailed(#[source] io::Error),

  #[error("{0}")]
  OpenFailed(#[source] io::Error),

  #[error("{0}")]
  ReadDirFailed(#[source] io::Error),

  #[error("{0}")]
  ProbeFailed(#[source] SymphoniaError),

  #[error("Track has no supported audio codec")]
  CodecNotSupported,

  #[error("{0}")]
  DecodingFailed(#[source] SymphoniaError),
}

/// Use the default symphonia probe and the path's extension as a `Hint`
///
/// This function is synchronous, so it must be called inside of `smol::unblock`
pub fn probe_track_sync(path: &Path) -> Result<ProbeResult, LoadTrackError> {
  let mut hint = Hint::new();
  if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
    hint.with_extension(extension);
  };

  // Use the default options for metadata and format readers.
  let meta_opts: MetadataOptions = Default::default();
  let fmt_opts: FormatOptions = FormatOptions {
    enable_gapless: true,
    ..Default::default()
  };

  let src = SyncFile::open(path).map_err(LoadTrackError::OpenFailed)?;
  let mss = MediaSourceStream::new(Box::new(src), Default::default());
  let probed = symphonia::default::get_probe()
    .format(&hint, mss, &fmt_opts, &meta_opts)
    .map_err(LoadTrackError::ProbeFailed)?;

  Ok(probed)
}

fn decode_first_frame_sync<'f, 'd>(
  format: &'f mut Box<dyn FormatReader>,
  decoder: &'d mut Box<dyn Decoder>,
  track_id: u32,
) -> Result<SignalSpec, LoadTrackError> {
  let decoded = loop {
    let current_span = match format.next_packet() {
      Ok(packet) => packet,
      Err(error) => return Err(LoadTrackError::DecodingFailed(error)),
    };

    // If the packet does not belong to the selected track, skip over it
    if current_span.track_id() != track_id {
      continue;
    }

    match decoder.decode(&current_span) {
      Ok(decoded) => break decoded,
      Err(error) => match error {
        SymphoniaError::DecodeError(_) => {
          // Decode errors are intentionally ignored with no retry limit.
          // This behavior ensures that the decoder skips over problematic packets
          // and continues processing the rest of the stream.
          continue;
        }
        _ => return Err(LoadTrackError::DecodingFailed(error)),
      },
    }
  };

  return Ok(decoded.spec().clone());
}

pub fn add_tag_to_metadata(metadata: &mut TrackMetadata, tag: &Tag) {
  match tag.std_key {
    Some(StandardTagKey::TrackTitle) => {
      if let Value::String(title) = &tag.value {
        metadata.title = Some(title.into());
      }
    }
    Some(StandardTagKey::Artist) => {
      if let Value::String(artist) = &tag.value {
        metadata.artists.insert(artist.into());
      }
    }
    Some(StandardTagKey::Album) => {
      if let Value::String(album) = &tag.value {
        metadata.album = Some(album.into());
      }
    }
    Some(StandardTagKey::TrackNumber) => {
      if let Value::UnsignedInt(track_number) = tag.value {
        metadata.track_number = Some(track_number as usize);
      } else if let Value::String(track_number) = &tag.value {
        if let Some(track_number) = track_number.parse().ok() {
          metadata.track_number = Some(track_number);
        }
      }
    }
    Some(StandardTagKey::Date) => {
      if let Value::String(date) = &tag.value {
        metadata.date = Some(date.into());
      }
    }
    Some(StandardTagKey::Genre) => {
      if let Value::String(genre) = &tag.value {
        metadata.genres.insert(genre.into());
      }
    }
    Some(StandardTagKey::Comment) => {
      if let Value::String(comment) = &tag.value {
        metadata.comments.push(comment.into());
      }
    }
    _ => (),
  }
}

fn update_metadata(metadata: &mut TrackMetadata, metadata_log: &mut Metadata) {
  loop {
    let Some(revision) = metadata_log.current() else {
      return;
    };

    for tag in revision.tags() {
      add_tag_to_metadata(metadata, tag);
    }

    if !metadata_log.is_latest() {
      metadata_log.pop();
    } else {
      break;
    }
  }
}

/// Load a `Track` from a specified file path
/// This will attempt to decode the first audio packet to ensure a correct `AudioSpec`
pub async fn load_file(path: PathBuf) -> Result<Track, LoadTrackError> {
  let outer_path = path.clone();

  let (audio_spec, metadata) = smol::unblock(move || {
    let mut probed = probe_track_sync(&path)?;

    let audio_track = probed
      .format
      .tracks()
      .iter()
      .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
      .ok_or(LoadTrackError::CodecNotSupported)?;
    let track_id = audio_track.id;

    let codec_params = &audio_track.codec_params;

    let bit_depth = codec_params.bits_per_sample;
    let total_duration = codec_params
      .time_base
      .zip(codec_params.n_frames)
      .map(|(base, spans)| base.calc_time(spans).into());

    let mut decoder = symphonia::default::get_codecs()
      .make(&audio_track.codec_params, &DecoderOptions::default())
      .map_err(|_| LoadTrackError::CodecNotSupported)?;

    let signal_spec = decode_first_frame_sync(&mut probed.format, &mut decoder, track_id)?;

    let audio_spec = AudioSpec {
      track_id,
      bit_depth,
      channel_bitmask: signal_spec.channels.bits(),
      channel_count: signal_spec.channels.count() as u16,
      sample_rate: signal_spec.rate,
      total_duration,
    };

    let mut track_metadata = Default::default();

    if let Some(mut metadata) = probed.metadata.get() {
      update_metadata(&mut track_metadata, &mut metadata)
    }

    update_metadata(&mut track_metadata, &mut probed.format.metadata());

    Ok((audio_spec, track_metadata))
  })
  .await?;

  Ok(Track::new(outer_path.clone(), audio_spec, metadata))
}

pub async fn get_cannonical_track_path(
  path: PathBuf,
) -> Result<PathBuf, (PathBuf, LoadTrackError)> {
  fs::canonicalize(&path)
    .await
    .map_err(|error| (path, LoadTrackError::CannonicalizeFailed(error)))
}

/// Returns the cannonical paths of all tracks in a directory
async fn search_directory(
  directory_path: PathBuf,
) -> impl Stream<Item = Result<PathBuf, (PathBuf, LoadTrackError)>> {
  fn_stream(async |emitter| {
    let mut entries = match fs::read_dir(&directory_path).await {
      Ok(files) => files,
      Err(error) => {
        return emitter
          .emit(Err((directory_path, LoadTrackError::ReadDirFailed(error))))
          .await;
      }
    };

    while let Some(entry) = entries.next().await {
      let path = match entry {
        Ok(entry) => entry.path(),
        Err(error) => {
          return emitter
            .emit(Err((
              directory_path.clone(),
              LoadTrackError::ReadDirFailed(error),
            )))
            .await;
        }
      };

      emitter.emit(get_cannonical_track_path(path).await).await;
    }
  })
}

/// Returns the cannonical paths of all tracks if a directory, or a single cannonical path if a file
pub async fn search_file_or_directory(
  path: PathBuf,
) -> impl Stream<Item = Result<PathBuf, (PathBuf, LoadTrackError)>> {
  fn_stream(async |emitter| {
    let metadata = match fs::metadata(&path).await {
      Ok(metadata) => metadata,
      Err(error) => {
        return emitter
          .emit(Err((path, LoadTrackError::OpenFailed(error))))
          .await;
      }
    };

    if metadata.is_dir() {
      let mut paths = pin!(search_directory(path).await);
      while let Some(path) = paths.next().await {
        emitter.emit(path).await;
      }
    } else {
      emitter.emit(get_cannonical_track_path(path).await).await;
    }
  })
}
