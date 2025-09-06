use std::{
  ffi::OsStr,
  os::unix::ffi::OsStrExt,
  path::{Component, Path, PathBuf},
  time::Duration,
};

use hsm_ipc::{LoopMode, PlaybackState, Track};
use mpris_server::zbus::zvariant::ObjectPath;

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

pub fn generate_metadata(track: &Track) -> mpris_server::Metadata {
  let track_id = ObjectPath::from_static_str_unchecked("/dev/djlaser/HomeSlashMusic/DefaultTrack");

  let metadata = track.metadata().clone();
  let mut builder = mpris_server::Metadata::builder()
    .trackid(track_id)
    .artist(metadata.artists)
    .genre(metadata.genres)
    .comment(metadata.comments);

  if let Some(title) = metadata.title {
    builder = builder.title(title);
  }

  if let Some(album) = metadata.album {
    builder = builder.album(album);
  }

  if let Some(track_number) = metadata.track_number {
    builder = builder.track_number(track_number as i32);
  }

  if let Some(date) = metadata.date {
    builder = builder.content_created(date);
  }

  if let Some(duration) = track.audio_spec().total_duration {
    builder = builder.length(mpris_server::Time::from_micros(duration.as_micros() as i64));
  }

  let url = encode_file_url(track.file_path());
  builder = builder.url(url);

  builder.build()
}

pub fn encode_file_url(path: &Path) -> String {
  let mut file_url = "file://".to_owned();
  for component in path.components() {
    match component {
      Component::Normal(os_str) => {
        file_url.push('/');
        file_url.push_str(&urlencoding::encode_binary(os_str.as_bytes()));
      }
      _ => (),
    }
  }

  file_url
}

pub fn decode_file_url(file_url: String) -> Option<PathBuf> {
  let encoded_file_path = file_url.strip_prefix("file://")?;
  let file_path = urlencoding::decode_binary(encoded_file_path.as_bytes());

  Some(PathBuf::from(OsStr::from_bytes(&file_path)))
}
