use std::{
  fs::File as SyncFile,
  path::{Path, PathBuf},
};

use hsm_ipc::TrackMetadata;
use symphonia::core::{
  codecs::CODEC_TYPE_NULL,
  formats::FormatOptions,
  io::MediaSourceStream,
  meta::{Metadata, MetadataOptions, StandardTagKey, Tag, Value},
  probe::{Hint, ProbeResult},
};

use super::LoadTrackError;

/// Use the default symphonia probe and the path's extension as a `Hint`
///
/// This function is synchronous, so it must be called inside of `smol::unblock`
fn probe_track_sync(path: &Path) -> Result<ProbeResult, LoadTrackError> {
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

fn add_tag_to_metadata(metadata: &mut TrackMetadata, tag: &Tag) {
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
pub async fn load_file(path: PathBuf) -> Result<TrackMetadata, LoadTrackError> {
  let track_metadata = smol::unblock(move || {
    let mut probed = probe_track_sync(&path)?;

    let audio_track = probed
      .format
      .tracks()
      .iter()
      .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
      .ok_or(LoadTrackError::CodecNotSupported)?;

    let codec_params = &audio_track.codec_params;

    let total_duration = codec_params
      .time_base
      .zip(codec_params.n_frames)
      .map(|(base, spans)| base.calc_time(spans).into());

    let mut track_metadata = TrackMetadata::new_empty(path);
    track_metadata.total_duration = total_duration;

    if let Some(mut metadata) = probed.metadata.get() {
      update_metadata(&mut track_metadata, &mut metadata)
    }

    update_metadata(&mut track_metadata, &mut probed.format.metadata());

    Ok(track_metadata)
  })
  .await?;

  Ok(track_metadata)
}
