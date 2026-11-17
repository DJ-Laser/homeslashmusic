use float_cmp::approx_eq;
use hsm_ipc::{InsertPosition, SeekOffset, QueueItemId};
use mpris_server::{
    PlayerInterface, RootInterface, TrackListInterface,
    zbus::{self, fdo},
};
use smol::channel::{self};

use crate::{
    AppHandle,
    audio::PlayerError,
    mpris::conversions::{
        as_dbus_time, as_loop_status, as_playback_status, decode_file_url, from_dbus_time,
        from_dbus_track_id, from_loop_status, from_real_dbus_track_id, generate_empty_metadata,
        generate_metadata, into_dbus_track_id,
    },
};

impl From<PlayerError> for fdo::Error {
    fn from(value: PlayerError) -> Self {
        Self::Failed(value.to_string())
    }
}

pub struct MprisImpl {
    handle: AppHandle,
    quit_tx: channel::Sender<()>,
}

impl MprisImpl {
    pub fn new(handle: AppHandle, quit_tx: channel::Sender<()>) -> Self {
        Self { handle, quit_tx }
    }

    fn unsupported<T>(message: &str) -> fdo::Result<T> {
        Err(fdo::Error::NotSupported(message.into()))
    }

    fn unsupported_set<T>(message: &str) -> zbus::Result<T> {
        Self::unsupported(message).map_err(zbus::Error::from)
    }

    fn channel_closed_error<T>(_t: T) -> fdo::Error {
        fdo::Error::Failed("Channel was unexpectedly closed".into())
    }
}

impl MprisImpl {
    async fn load_uri(
        &self,
        uri: String,
        _position: InsertPosition,
        _set_as_current: bool,
    ) -> fdo::Result<()> {
        let Some(_file_path) = decode_file_url(&uri) else {
            return Self::unsupported("Unsupported uri type");
        };

        Self::unsupported("Not yet implemented")
    }
}

impl RootInterface for MprisImpl {
    async fn raise(&self) -> fdo::Result<()> {
        Self::unsupported("Raise is not supported")
    }

    async fn quit(&self) -> fdo::Result<()> {
        match self.quit_tx.try_send(()) {
            Err(channel::TrySendError::Closed(e)) => Err(Self::channel_closed_error(e)),

            // If the channel is full, a quit message was already sent
            Ok(()) | Err(channel::TrySendError::Full(())) => Ok(()),
        }
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Self::unsupported_set("Fullscreen is not supported")
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("~/Music".into())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("homeslashmusic".into())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["file".into()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![
            "audio/aac".into(),
            "audio/mpeg".into(),
            "audio/ogg".into(),
            "audio/wav".into(),
        ])
    }
}

impl PlayerInterface for MprisImpl {
    async fn next(&self) -> fdo::Result<()> {
        self.handle.player.go_to_next_track().await?;
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        self.handle.player.go_to_previous_track(true).await?;
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.handle.player.pause();
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.handle.player.pause();
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.handle.player.stop();
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        self.handle.player.play().await;
        Ok(())
    }

    async fn seek(&self, offset: mpris_server::Time) -> fdo::Result<()> {
        if offset.is_zero() {
            return Ok(());
        }

        let seek_offset = if offset.is_positive() {
            SeekOffset::Forward(from_dbus_time(offset))
        } else {
            SeekOffset::Backward(from_dbus_time(offset))
        };

        self.handle.player.seek_current(seek_offset).await?;
        Ok(())
    }

    async fn set_position(
        &self,
        track_id: mpris_server::TrackId,
        position: mpris_server::Time,
    ) -> fdo::Result<()> {
        if position.is_negative() {
            return Ok(());
        }

        let track_id = from_real_dbus_track_id(&track_id)?;

        self.handle
            .player
            .seek_to(track_id, from_dbus_time(position))
            .await?;

        Ok(())
    }

    async fn open_uri(&self, uri: String) -> fdo::Result<()> {
        self.load_uri(uri, InsertPosition::End, false).await
    }

    async fn playback_status(&self) -> fdo::Result<mpris_server::PlaybackStatus> {
        let playback_state = self.handle.player.playback_state();
        Ok(as_playback_status(playback_state))
    }

    async fn loop_status(&self) -> fdo::Result<mpris_server::LoopStatus> {
        let loop_mode = self.handle.player.loop_mode();
        Ok(as_loop_status(loop_mode))
    }

    async fn set_loop_status(&self, loop_status: mpris_server::LoopStatus) -> zbus::Result<()> {
        self.handle
            .player
            .set_loop_mode(from_loop_status(loop_status));
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<mpris_server::PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, rate: mpris_server::PlaybackRate) -> zbus::Result<()> {
        if approx_eq!(f64, rate, 0.0) {
            self.pause().await?;
        } else if approx_eq!(f64, rate, 1.0) {
            Self::unsupported_set("Unsupported rate")?;
        }

        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(self.handle.player.shuffle())
    }

    async fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
        self.handle.player.set_shuffle(shuffle).await;
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<mpris_server::Metadata> {
        let metadata = match self.handle.player.current_track() {
            Some(track) => generate_metadata(track.clone_owned_instance()),
            None => mpris_server::Metadata::builder()
                .trackid(mpris_server::TrackId::NO_TRACK)
                .build(),
        };

        Ok(metadata)
    }

    async fn volume(&self) -> fdo::Result<mpris_server::Volume> {
        Ok(self.handle.player.volume().into())
    }

    async fn set_volume(&self, volume: mpris_server::Volume) -> zbus::Result<()> {
        #[expect(clippy::cast_possible_truncation)]
        self.handle.player.set_volume(volume as f32);
        Ok(())
    }

    async fn position(&self) -> fdo::Result<mpris_server::Time> {
        Ok(as_dbus_time(self.handle.player.position()))
    }

    async fn minimum_rate(&self) -> fdo::Result<mpris_server::PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<mpris_server::PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

impl TrackListInterface for MprisImpl {
    async fn get_tracks_metadata(
        &self,
        track_ids: Vec<mpris_server::TrackId>,
    ) -> fdo::Result<Vec<mpris_server::Metadata>> {
        let track_ids = track_ids
            .iter()
            .map(from_real_dbus_track_id)
            .collect::<Result<Vec<QueueItemId>, _>>()?;

        let mut tracks = self.handle.player.get_tracks();

        let metadata_list = track_ids
            .into_iter()
            .map(
                |track_id| match tracks.iter().position(|track| track.id == track_id) {
                    Some(index) => {
                        let track = tracks.swap_remove(index);
                        generate_metadata(track.clone_owned_instance())
                    }
                    None => generate_empty_metadata(track_id),
                },
            )
            .collect();

        Ok(metadata_list)
    }

    async fn add_track(
        &self,
        uri: mpris_server::Uri,
        after_track: mpris_server::TrackId,
        set_as_current: bool,
    ) -> fdo::Result<()> {
        let after_track = from_dbus_track_id(&after_track)?;
        let insert_position = match after_track {
            Some(track_id) => InsertPosition::After(track_id),
            None => InsertPosition::Start,
        };

        self.load_uri(uri, insert_position, set_as_current).await?;

        Ok(())
    }

    async fn remove_track(&self, _track_id: mpris_server::TrackId) -> fdo::Result<()> {
        todo!()
    }

    async fn go_to(&self, track_id: mpris_server::TrackId) -> fdo::Result<()> {
        let track_id = from_real_dbus_track_id(&track_id)?;
        self.handle.player.go_to_track(track_id).await;
        Ok(())
    }

    async fn tracks(&self) -> fdo::Result<Vec<mpris_server::TrackId>> {
        let track_ids = self.handle.player.get_tracks();

        let track_ids = track_ids
            .into_iter()
            .map(|track| into_dbus_track_id(track.id))
            .collect();

        Ok(track_ids)
    }

    async fn can_edit_tracks(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}
