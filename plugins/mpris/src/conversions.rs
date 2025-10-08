use std::{
  ffi::OsStr,
  os::unix::ffi::OsStrExt,
  path::{Component, Path, PathBuf},
  time::Duration,
};

use hsm_ipc::{LoopMode, PlaybackState, TrackId, TrackInstance};
use mpris_server::zbus::{fdo, zvariant::ObjectPath};

pub fn as_playback_status(playback_state: PlaybackState) -> mpris_server::PlaybackStatus {
  match playback_state {
    PlaybackState::Playing => mpris_server::PlaybackStatus::Playing,
    PlaybackState::Paused => mpris_server::PlaybackStatus::Paused,
    PlaybackState::Stopped => mpris_server::PlaybackStatus::Stopped,
  }
}

pub fn as_loop_status(loop_mode: LoopMode) -> mpris_server::LoopStatus {
  match loop_mode {
    LoopMode::None => mpris_server::LoopStatus::None,
    LoopMode::Track => mpris_server::LoopStatus::Track,
    LoopMode::Playlist => mpris_server::LoopStatus::Playlist,
  }
}

pub fn from_loop_status(loop_status: mpris_server::LoopStatus) -> LoopMode {
  match loop_status {
    mpris_server::LoopStatus::None => LoopMode::None,
    mpris_server::LoopStatus::Track => LoopMode::Track,
    mpris_server::LoopStatus::Playlist => LoopMode::Playlist,
  }
}

pub fn as_dbus_time(time: Duration) -> mpris_server::Time {
  mpris_server::Time::from_micros(time.as_micros() as i64)
}

pub fn from_dbus_time(time: mpris_server::Time) -> Duration {
  Duration::from_micros(time.as_micros() as u64)
}

const TRACK_ID_PREFIX: &str = "/dev/djlaser/HomeSlashMusic/queue/";

pub fn into_dbus_track_id(track_id: TrackId) -> mpris_server::TrackId {
  let path = ObjectPath::try_from(format!("{TRACK_ID_PREFIX}{track_id}"))
    .expect("object path should be valid");

  mpris_server::TrackId::from(path)
}

/// Returns Err is the track_id is invalid
/// returns Ok(None) if the track id was mpris_server::TrackId::NO_TRACK
/// returns Ok(Some(...)) if the track id was a valid format, though it may not actually exist
pub fn from_dbus_track_id(track_id: mpris_server::TrackId) -> fdo::Result<Option<TrackId>> {
  if track_id == mpris_server::TrackId::NO_TRACK {
    return Ok(None);
  }

  if let Some(track_id) = track_id.strip_prefix(TRACK_ID_PREFIX)
    && let Ok(track_id) = track_id.parse()
  {
    Ok(Some(track_id))
  } else {
    Err(fdo::Error::InvalidArgs("Invalid track id".into()))
  }
}

/// Same as `from_dbus_track_id` except mpris_server::TrackId::NO_TRACK returns an invalid error instead of Ok(None)
pub fn from_real_dbus_track_id(track_id: mpris_server::TrackId) -> fdo::Result<TrackId> {
  if let Some(track_id) = from_dbus_track_id(track_id)? {
    Ok(track_id)
  } else {
    Err(fdo::Error::InvalidArgs(
      "/org/mpris/MediaPlayer2/TrackList/NoTrack is not a valid track id".into(),
    ))
  }
}

pub fn generate_empty_metadata(track_id: TrackId) -> mpris_server::Metadata {
  let track_id = into_dbus_track_id(track_id);
  mpris_server::Metadata::builder().trackid(track_id).build()
}

pub fn generate_metadata(track: TrackInstance) -> mpris_server::Metadata {
  let track_id = into_dbus_track_id(track.id);
  let track = track.metadata;

  let mut builder = mpris_server::Metadata::builder()
    .trackid(track_id)
    .artist(track.artists)
    .genre(track.genres)
    .comment(track.comments);

  if let Some(title) = track.title {
    builder = builder.title(title);
  }

  if let Some(album) = track.album {
    builder = builder.album(album);
  }

  if let Some(track_number) = track.track_number {
    builder = builder.track_number(track_number as i32);
  }

  if let Some(date) = track.date {
    builder = builder.content_created(date);
  }

  if let Some(duration) = track.total_duration {
    builder = builder.length(mpris_server::Time::from_micros(duration.as_micros() as i64));
  }

  let url = encode_file_url(&track.file_path);
  builder = builder.url(url);

  builder.build()
}

pub fn encode_file_url(path: &Path) -> String {
  let mut file_url = "file://".to_owned();
  for component in path.components() {
    if let Component::Normal(os_str) = component {
      file_url.push('/');
      file_url.push_str(&urlencoding::encode_binary(os_str.as_bytes()));
    }
  }

  file_url
}

pub fn decode_file_url(file_url: String) -> Option<PathBuf> {
  let encoded_file_path = file_url.strip_prefix("file://")?;
  let file_path = urlencoding::decode_binary(encoded_file_path.as_bytes());

  Some(PathBuf::from(OsStr::from_bytes(&file_path)))
}
