use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use float_cmp::approx_eq;
use hsm_ipc::{
    Event, InsertPosition, LoopMode, PlaybackState, QueuedTrackId, QueuedTrackMetadata, SeekOffset,
    SeekPosition,
};
use log::{debug, error};
use rodio::mixer::Mixer;
use smol::channel;
use thiserror::Error;

use crate::{
    audio::{
        controlled_source::{self, Controls, SeekError, SourceEvent, wrap_source},
        decoder::{LoadTrackError, TrackDecoder},
        output::{PlayerAudioOutput, SourceQueue, SourceQueueState},
        track_list::{ArcTrackInstance, TrackList},
    },
    database::TrackLocation,
};

#[derive(Debug, Error)]
pub enum PlayerError {
    /// Should never happen since the player managers both ends of the channel
    #[error("Internal Player Error: SourceEvent channel closed")]
    SourceChannelClosed,

    /// Since we use an unbounded channel, an error means it must be closed
    #[error("Event channel closed")]
    EventChannelClosed,

    #[error("Failed to load track: {0}")]
    LoadTrack(#[from] LoadTrackError),

    #[error("Track id {0} not in track list")]
    TrackIdNotFound(QueuedTrackId),

    #[error("failed to seek: ")]
    SeekFailed(#[from] SeekError),
}

impl PlayerError {
    pub fn is_recoverable(&self) -> bool {
        !matches!(self, Self::SourceChannelClosed | Self::EventChannelClosed)
    }
}

#[derive(Debug)]
pub struct Player {
    tracks: TrackList,
    current_track_index: AtomicUsize,

    controls: Arc<Controls>,
    source_queue: Arc<SourceQueue>,

    event_tx: async_broadcast::Sender<Event>,
    source_tx: channel::Sender<SourceEvent>,
    source_rx: channel::Receiver<SourceEvent>,

    #[expect(unused, reason = "Channel closes when dropped, waking output thread")]
    guard_tx: channel::Sender<()>,
}

impl Player {
    pub fn connect_new(
        event_tx: async_broadcast::Sender<Event>,
        mixer: &Mixer,
        guard_tx: channel::Sender<()>,
    ) -> Self {
        let (player, source) = Self::new(event_tx, guard_tx);
        mixer.add(source);
        player
    }

    pub fn new(
        event_tx: async_broadcast::Sender<Event>,
        guard_tx: channel::Sender<()>,
    ) -> (Self, PlayerAudioOutput) {
        let (source_tx, source_rx) = channel::unbounded();

        let player = Self {
            tracks: TrackList::new(),
            current_track_index: AtomicUsize::new(0),

            controls: Arc::new(Controls::new()),
            source_queue: Arc::new(SourceQueue::new()),

            event_tx,
            source_tx,
            source_rx,
            guard_tx,
        };

        let audio_source = PlayerAudioOutput::new(player.source_queue.clone());

        (player, audio_source)
    }

    fn emit(&self, event: Event) {
        if self.event_tx.try_broadcast(event).is_err() {
            error!("Player event channel closed");
        }
    }

    fn clear_source_queue(&self) {
        let mut source_queue = self.source_queue.lock();

        if source_queue.is_playing() {
            source_queue.invalidate();
            self.controls.skip_one();
        }
    }

    /// If `wait_for_empty_queue` is true, this function will loop until the queue become open for replacement
    /// If it is not ensured that the queue is empty beforehand it can cause an infinite loop and lockup
    async fn queue_track(&self, track: TrackLocation, wait_for_empty_queue: bool) {
        let decoder = match TrackDecoder::new(track).await {
            Ok(source) => source,
            Err(error) => {
                error!("Failed to load track: {error}");
                return;
            }
        };

        loop {
            {
                let mut source_queue = self.source_queue.lock();
                if wait_for_empty_queue || !source_queue.is_queued() {
                    let source = Box::new(wrap_source(
                        decoder,
                        self.controls.clone(),
                        self.source_tx.clone(),
                    ));

                    *source_queue = SourceQueueState::Queued(source);
                    break;
                }
            }

            smol::Timer::after(controlled_source::SOURCE_UPDATE_INTERVAL * 3).await;
        }
    }

    /// Returns true if there was a current track to queue
    ///
    /// If `use_queued` is true this function will use the source waiting in queue instead of reloading the current track
    /// Because this function queues the next track, `use_queued` should only be true if the `current_track_index` is exactly one more
    /// than the last call to `queue_current_track`
    async fn queue_current_track(&self, use_queued: bool) -> bool {
        let Some((current_track, next_track)) = self
            .tracks
            .get_tracks_to_queue(self.current_track_index.load(Ordering::Acquire))
        else {
            return false;
        };

        let load_necessary = {
            let mut source_queue = self.source_queue.lock();
            match *source_queue {
                // Skip the current track so the queued one plays
                SourceQueueState::Queued(_) => {
                    self.controls.skip_one();
                    if !use_queued {
                        source_queue.invalidate();
                    }

                    !use_queued
                }

                // No queued track means the current track ended and the next (queued) track began playing
                // If `use_queued` is false skip and load a new track
                SourceQueueState::Playing => {
                    if !use_queued {
                        self.controls.skip_one();
                    }

                    !use_queued
                }

                // No track playing, load the current track
                SourceQueueState::None => true,
            }
        };

        if load_necessary {
            self.queue_track(current_track.loader.clone(), true).await;
        }

        if let Some(next_track) = next_track {
            self.queue_track(next_track.loader.clone(), false).await;
        }

        true
    }

    pub fn playback_state(&self) -> PlaybackState {
        self.controls.playback_state()
    }

    fn is_stopped(&self) -> bool {
        matches!(self.playback_state(), PlaybackState::Stopped)
    }

    fn set_playback_state(&self, new_state: PlaybackState) -> PlaybackState {
        let prev_state = self.controls.set_playback_state(new_state);

        if prev_state != new_state {
            self.emit(Event::PlaybackStateChanged(new_state));
            debug!("Setting playback state to {new_state:?}");
        }

        prev_state
    }

    pub async fn play(&self) {
        if self.is_stopped() {
            let had_tracks = self.queue_current_track(true);
            if !had_tracks.await {
                return;
            }
        }

        self.set_playback_state(PlaybackState::Playing);
    }

    pub fn pause(&self) {
        let prev_state = self.controls.playback_state();

        // Don't un-stop playback on pause
        if matches!(prev_state, PlaybackState::Playing) {
            self.set_playback_state(PlaybackState::Paused);
        }
    }

    pub async fn toggle_playback(&self) {
        let current_state = self.controls.playback_state();
        match current_state {
            PlaybackState::Paused | PlaybackState::Stopped => self.play().await,
            PlaybackState::Playing => self.pause(),
        }
    }

    pub fn stop(&self) {
        self.clear_source_queue();
        self.set_playback_state(PlaybackState::Stopped);
        self.controls.reset_position();
    }

    pub fn current_track(&self) -> Option<ArcTrackInstance> {
        self.tracks
            .get_track_at(self.current_track_index.load(Ordering::Acquire))
    }

    pub async fn go_to_track(&self, track_id: QueuedTrackId) -> bool {
        let Some(track_index) = self.tracks.get_track_index(track_id) else {
            return false;
        };

        self.current_track_index
            .store(track_index, Ordering::Release);

        if !self.is_stopped() {
            self.queue_current_track(false).await;
        }

        true
    }

    pub async fn go_to_index(&self, index: usize) -> bool {
        if index >= self.tracks.len() {
            return false;
        }

        if !self.is_stopped() {
            self.queue_current_track(true).await;
        }

        true
    }

    async fn stop_or_wrap_track(&self, reverse: bool) {
        let printed_position = if reverse { "beginning" } else { "end" };
        let printed_loop_position = if reverse { "end" } else { "beginning" };

        let should_loop = !matches!(self.controls.loop_mode(), LoopMode::None);

        // Don't skip to end if loop is off
        let new_index = if should_loop && reverse {
            self.tracks.len() - 1
        } else {
            0
        };

        self.current_track_index.store(new_index, Ordering::Release);

        if should_loop {
            debug!("Track list reached {printed_position}, looping to {printed_loop_position}");

            if !self.is_stopped() {
                self.queue_current_track(false).await;
            }
        } else {
            debug!("Track list reached {printed_position}, stopping");
            self.stop();
        }
    }

    pub async fn go_to_next_track(&self) -> Result<(), PlayerError> {
        let new_index = 1 + self.current_track_index.fetch_add(1, Ordering::Release);

        if self.is_stopped() {
            if new_index >= self.tracks.len() {
                self.stop_or_wrap_track(false).await;
            }

            return Ok(());
        }

        if !self.queue_current_track(true).await {
            self.stop_or_wrap_track(false).await;
        }

        Ok(())
    }

    pub async fn go_to_previous_track(&self) -> Result<(), PlayerError> {
        const RESTART_THRESHOLD: Duration = Duration::from_secs(5);

        if self.position() > RESTART_THRESHOLD {
            self.seek(SeekPosition::To(Duration::ZERO)).await
        } else {
            let current_index = self.current_track_index.load(Ordering::Acquire);

            if current_index == 0 {
                self.stop_or_wrap_track(true).await;
            } else {
                self.current_track_index
                    .store(current_index - 1, Ordering::Release);

                if !self.is_stopped() {
                    self.queue_current_track(false).await;
                }
            }

            Ok(())
        }
    }

    pub fn shuffle(&self) -> bool {
        self.tracks.shuffle_enabled()
    }

    pub async fn set_shuffle(&self, shuffle: bool) {
        let prev_shuffle = self.shuffle();
        if shuffle != prev_shuffle {
            let current_index = self.current_track_index.load(Ordering::Acquire);
            let new_index = self.tracks.set_shuffle(shuffle, current_index);

            self.current_track_index.store(new_index, Ordering::Release);

            self.emit(Event::ShuffleChanged(shuffle));
            debug!("Shuffle set to {shuffle}");

            if !self.is_stopped() {
                let current_track_index = self.current_track_index.load(Ordering::Acquire);
                if let Some((_, Some(next_track))) =
                    self.tracks.get_tracks_to_queue(current_track_index)
                {
                    self.queue_track(next_track.loader.clone(), true).await;
                }
            }
        }
    }

    pub fn loop_mode(&self) -> LoopMode {
        self.controls.loop_mode()
    }

    pub fn set_loop_mode(&self, loop_mode: LoopMode) {
        let prev_mode = self.controls.set_loop_mode(loop_mode);

        if loop_mode != prev_mode {
            self.emit(Event::LoopModeChanged(loop_mode));
            debug!("Loop mode set to {loop_mode:?}");
        }
    }

    pub fn volume(&self) -> f32 {
        self.controls.volume()
    }

    pub fn set_volume(&self, volume: f32) {
        let clamped_volume = volume.clamp(0.0, 1.0);
        let prev_volume = self.controls.set_volume(clamped_volume);

        if approx_eq!(f32, clamped_volume, prev_volume) {
            self.emit(Event::VolumeChanged(clamped_volume));
            debug!("volume set to {volume:?}");
        }
    }

    pub fn position(&self) -> Duration {
        self.controls.position()
    }

    pub async fn seek(&self, seek_position: SeekPosition) -> Result<(), PlayerError> {
        if matches!(*self.source_queue.lock(), SourceQueueState::None) {
            return Ok(());
        }

        self.controls
            .seek(seek_position)
            .await
            .map_err(|_| SeekError::ErrorChannelClosed)??;

        debug!("Seeked {seek_position:?}");

        Ok(())
    }

    pub async fn seek_current(&self, seek_offset: SeekOffset) -> Result<(), PlayerError> {
        self.seek(seek_offset.into()).await
    }

    pub async fn seek_to(
        &self,
        track_id: QueuedTrackId,
        position: Duration,
    ) -> Result<bool, PlayerError> {
        if let Some(track) = self.current_track()
            && track.id == track_id
        {
            self.seek(SeekPosition::To(position)).await?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn get_tracks(&self) -> Vec<ArcTrackInstance> {
        self.tracks.get_tracks()
    }

    pub fn clear_tracks(&self) {
        self.stop();
        self.tracks.clear();
        self.current_track_index.store(0, Ordering::Release);
        debug!("Clearing track list");
    }

    /// Inserts new tracks at a specified position in the track list
    pub async fn insert_tracks(
        &self,
        position: InsertPosition,
        tracks: Vec<(TrackLocation, Arc<QueuedTrackMetadata>)>,
        set_as_current: bool,
    ) -> Result<Option<QueuedTrackId>, PlayerError> {
        let current_index = self.current_track_index.load(Ordering::Acquire);

        let (new_current_index, first_id) =
            self.tracks
                .insert_tracks(current_index, position, tracks, set_as_current)?;

        self.current_track_index
            .store(new_current_index, Ordering::Release);

        // If the track list was replaced, a new song must begin playing
        if set_as_current && !self.is_stopped() {
            self.queue_current_track(false).await;
        }

        Ok(first_id)
    }

    pub async fn run(&self) -> Result<(), PlayerError> {
        loop {
            let event = self
                .source_rx
                .recv()
                .await
                .map_err(|_| PlayerError::SourceChannelClosed)?;

            if event.indicates_end()
                && !matches!(event, SourceEvent::Skipped)
                && let Err(error) = self.go_to_next_track().await
            {
                if error.is_recoverable() {
                    error!("{error}");
                } else {
                    return Err(error);
                }
            }

            match event {
                SourceEvent::LoopError(error) => error!("Error looping source: {error}"),
                SourceEvent::Seeked(position) => self.emit(Event::Seeked(position)),
                _ => (),
            }
        }
    }
}
