use std::{
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use hsm_ipc::{LoopMode, PlaybackState, QueuedTrack, QueuedTrackId};
use mpris_server::zbus::{ObjectPath, fdo};

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
    mpris_server::Time::from_micros(i64::try_from(time.as_micros()).unwrap_or(i64::MAX))
}

pub fn from_dbus_time(time: mpris_server::Time) -> Duration {
    Duration::from_micros(u64::try_from(time.as_micros()).unwrap_or(0))
}

const TRACK_ID_PREFIX: &str = "/dev/djlaser/HomeSlashMusic/queue/";

pub fn into_dbus_track_id(track_id: QueuedTrackId) -> mpris_server::TrackId {
    let path = ObjectPath::try_from(format!("{TRACK_ID_PREFIX}{track_id}"))
        .expect("TRACK_ID_PREFIX should always make valid object paths");

    mpris_server::TrackId::from(path)
}

/// Returns Err is the `track_id` is invalid
/// returns Ok(None) if the track id was `mpris_server::TrackId::NO_TRACK`
/// returns Ok(Some(...)) if the track id was a valid format, though it may not actually exist
pub fn from_dbus_track_id(track_id: &mpris_server::TrackId) -> fdo::Result<Option<QueuedTrackId>> {
    if *track_id == mpris_server::TrackId::NO_TRACK {
        return Ok(None);
    }

    if let Some(track_id) = track_id.strip_prefix(TRACK_ID_PREFIX)
        && let Ok(track_id) = track_id.parse()
    {
        Ok(Some(QueuedTrackId(track_id)))
    } else {
        Err(fdo::Error::InvalidArgs("Invalid track id".into()))
    }
}

/// Same as `from_dbus_track_id` except `mpris_server::TrackId::NO_TRACK` returns an invalid error instead of Ok(None)
pub fn from_real_dbus_track_id(track_id: &mpris_server::TrackId) -> fdo::Result<QueuedTrackId> {
    if let Some(track_id) = from_dbus_track_id(track_id)? {
        Ok(track_id)
    } else {
        Err(fdo::Error::InvalidArgs(
            "/org/mpris/MediaPlayer2/TrackList/NoTrack is not a valid track id".into(),
        ))
    }
}

pub fn generate_empty_metadata(track_id: QueuedTrackId) -> mpris_server::Metadata {
    let track_id = into_dbus_track_id(track_id);
    mpris_server::Metadata::builder().trackid(track_id).build()
}

pub fn generate_metadata(track: QueuedTrack) -> mpris_server::Metadata {
    let track_id = into_dbus_track_id(track.id);
    let track = track.metadata;

    let mut builder = mpris_server::Metadata::builder()
        .trackid(track_id)
        .artist(track.artist_text)
        .genre(track.genres);

    builder = builder.title(track.title);

    if let Some(album) = track.album {
        builder = builder.album(album);
    }

    if let Some(track_number) = track.track_number {
        builder = builder.track_number(i32::try_from(track_number).unwrap_or(i32::MAX));
    }

    if let Some(date) = track.date {
        builder = builder.content_created(date);
    }

    builder = builder.length(as_dbus_time(track.length));

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

pub fn decode_file_url(file_url: &str) -> Option<PathBuf> {
    let encoded_file_path = file_url.strip_prefix("file://")?;
    let file_path = urlencoding::decode_binary(encoded_file_path.as_bytes());

    Some(PathBuf::from(OsStr::from_bytes(&file_path)))
}
