use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use hsm_ipc::{LoopMode, PlaybackState, SeekPosition};
use rodio::{
    Source,
    source::{Amplify, Pausable, SeekError as RodioSeekError, TrackPosition},
};
use smol::channel::Sender;
use thiserror::Error;

type SeekResult = Result<(), SeekError>;

const fn encode_playback_state(playback_state: PlaybackState) -> u8 {
    match playback_state {
        PlaybackState::Playing => 0,
        PlaybackState::Paused => 1,
        PlaybackState::Stopped => 2,
    }
}

const fn decode_playback_state(val: u8) -> PlaybackState {
    match val {
        0 => PlaybackState::Playing,
        1 => PlaybackState::Paused,
        _ => PlaybackState::Stopped,
    }
}

const fn encode_loop_mode(loop_mode: LoopMode) -> u8 {
    match loop_mode {
        LoopMode::None => 0,
        LoopMode::Track => 1,
        LoopMode::Playlist => 2,
    }
}

const fn decode_loop_mode(val: u8) -> LoopMode {
    match val {
        0 => LoopMode::None,
        1 => LoopMode::Track,
        _ => LoopMode::Playlist,
    }
}

type SeekRequest = (SeekPosition, async_oneshot::Sender<SeekResult>);

#[derive(Debug)]
pub struct Controls {
    pub playback_state: AtomicU8,
    pub loop_mode: AtomicU8,
    pub volume: AtomicU32,
    pub to_skip: AtomicUsize,
    pub position_nanos: AtomicU64,
    pub seek_position: Mutex<Option<SeekRequest>>,
}

impl Controls {
    pub fn new() -> Self {
        Self {
            playback_state: AtomicU8::new(encode_playback_state(PlaybackState::Stopped)),
            loop_mode: AtomicU8::new(encode_loop_mode(LoopMode::None)),
            to_skip: AtomicUsize::new(0),
            volume: AtomicU32::new(1.0f32.to_bits()),
            position_nanos: AtomicU64::new(0),
            seek_position: Mutex::new(None),
        }
    }

    pub fn playback_state(&self) -> PlaybackState {
        decode_playback_state(self.playback_state.load(Ordering::Acquire))
    }

    pub fn set_playback_state(&self, playback_state: PlaybackState) -> PlaybackState {
        decode_playback_state(
            self.playback_state
                .swap(encode_playback_state(playback_state), Ordering::AcqRel),
        )
    }

    pub fn loop_mode(&self) -> LoopMode {
        decode_loop_mode(self.loop_mode.load(Ordering::Acquire))
    }

    pub fn set_loop_mode(&self, loop_mode: LoopMode) -> LoopMode {
        decode_loop_mode(
            self.loop_mode
                .swap(encode_loop_mode(loop_mode), Ordering::AcqRel),
        )
    }

    pub fn skip_one(&self) {
        self.to_skip.fetch_add(1, Ordering::AcqRel);
    }

    pub fn skip_current(&self) {
        self.to_skip.store(1, Ordering::Release);
    }

    pub fn consume_skip(&self) -> bool {
        self.to_skip
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
            .is_ok()
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Acquire))
    }

    pub fn set_volume(&self, volume: f32) -> f32 {
        f32::from_bits(self.volume.swap(volume.to_bits(), Ordering::AcqRel))
    }

    pub fn position(&self) -> Duration {
        Duration::from_nanos(self.position_nanos.load(Ordering::Acquire))
    }

    pub fn set_position(&self, position: Duration) {
        let nanos = u64::try_from(position.as_nanos()).unwrap_or(u64::MAX);
        self.position_nanos.store(nanos, Ordering::Release);
    }

    pub fn reset_position(&self) {
        self.position_nanos.store(0, Ordering::Release);
    }

    pub fn seek(&self, seek_position: SeekPosition) -> async_oneshot::Receiver<SeekResult> {
        let (tx, rx) = async_oneshot::oneshot();
        *self.seek_position.lock().unwrap() = Some((seek_position, tx));
        rx
    }

    pub fn take_seek(&self) -> Option<SeekRequest> {
        self.seek_position.try_lock().ok()?.take()
    }
}

#[derive(Debug)]
pub struct TrackGainFactors(AtomicU64);

impl TrackGainFactors {
    pub fn encode(factor: f32, max_factor: f32) -> u64 {
        let lo = u64::from(factor.to_bits());
        let hi = u64::from(max_factor.to_bits());
        lo | (hi << 32)
    }

    pub fn decode(val: u64) -> (f32, f32) {
        #[expect(clippy::cast_possible_truncation, reason = "Get lower 32 bits")]
        let factor = f32::from_bits(val as u32);
        let max_factor = f32::from_bits((val >> 32) as u32);
        (factor, max_factor)
    }

    pub fn new(factor: f32, max_factor: f32) -> Self {
        Self(AtomicU64::new(Self::encode(factor, max_factor)))
    }

    pub fn set(&self, factor: f32, max_factor: f32) {
        self.0
            .store(Self::encode(factor, max_factor), Ordering::Release);
    }

    pub fn get(&self) -> (f32, f32) {
        Self::decode(self.0.load(Ordering::Acquire))
    }
}

#[derive(Debug)]
pub enum SourceEvent {
    Seeked(Duration),
    LoopError(RodioSeekError),
    Finished,
    Skipped,
    Looped,
}

impl SourceEvent {
    /// If this event indicates that the soutrce has ended.
    /// Used to manage the player's internal source count
    pub fn indicates_end(&self) -> bool {
        matches!(self, Self::Finished | Self::LoopError(_) | Self::Skipped)
    }
}

type ControlledSourceInner<S> = Pausable<Amplify<TrackPosition<S>>>;

pub const SOURCE_UPDATE_INTERVAL: Duration = Duration::from_millis(5);

struct ControlledSource<I> {
    input: ControlledSourceInner<I>,
    controls: Arc<Controls>,
    source_tx: Sender<SourceEvent>,
    gain_factors: Arc<TrackGainFactors>,

    should_skip: bool,
    cached_loop_mode: LoopMode,
    ended: bool,
}

impl<I> ControlledSource<I>
where
    I: Source,
{
    pub fn new(
        input: I,
        controls: Arc<Controls>,
        source_tx: Sender<SourceEvent>,
        gain_factors: Arc<TrackGainFactors>,
    ) -> Self {
        Self {
            input: input.track_position().amplify(1.0).pausable(false),
            controls,
            source_tx,
            gain_factors,

            should_skip: false,
            cached_loop_mode: LoopMode::None,
            ended: false,
        }
    }

    fn update_control_state(&mut self) {
        let controls = &self.controls;

        self.cached_loop_mode = controls.loop_mode();
        if controls.consume_skip() {
            self.should_skip = true;
            return;
        }

        let pausable = &mut self.input;
        pausable.set_paused(matches!(controls.playback_state(), PlaybackState::Paused));

        let volume_controlled = pausable.inner_mut();
        volume_controlled.set_factor({
            let volume_factor = f32::powi(controls.volume(), 3);
            let (gain_factor, max_factor) = self.gain_factors.get();
            f32::min(volume_factor * gain_factor, max_factor)
        });

        let position_tracked = volume_controlled.inner_mut();
        if let Some((seek_position, mut tx)) = controls.take_seek() {
            let current_position = position_tracked.get_pos();
            let seek_position = match seek_position {
                SeekPosition::Forward(duration) => current_position.saturating_add(duration),
                SeekPosition::Backward(duration) => current_position.saturating_sub(duration),
                SeekPosition::To(position) => position,
            };

            let res = position_tracked
                .try_seek(seek_position)
                .map_err(|error| SeekError::SeekFailed(error.to_string()));

            if res.is_ok() {
                let _ = self.source_tx.try_send(SourceEvent::Seeked(seek_position));
            }

            let _ = tx.send(res);
        }

        controls.set_position(position_tracked.get_pos());
    }
}

impl<I> Iterator for ControlledSource<I>
where
    I: Source,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }

        if self.should_skip {
            let _ = self.source_tx.try_send(SourceEvent::Skipped);
            self.ended = true;

            return None;
        }

        if let Some(value) = self.input.next() {
            return Some(value);
        }

        let sample = if matches!(self.cached_loop_mode, LoopMode::Track) {
            if let Err(error) = self.input.try_seek(Duration::ZERO) {
                let _ = self.source_tx.try_send(SourceEvent::LoopError(error));
                self.ended = true;

                return None;
            }

            let _ = self.source_tx.try_send(SourceEvent::Looped);
            self.input.next()
        } else {
            None
        };

        if sample.is_none() {
            let _ = self.source_tx.try_send(SourceEvent::Finished);
            self.ended = true;
        }

        sample
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

#[derive(Debug, Error)]
pub enum SeekError {
    #[error("Internal Player Error: SeekError channel closed")]
    ErrorChannelClosed,

    #[error("{0}")]
    SeekFailed(String),
}

impl<I> Source for ControlledSource<I>
where
    I: Source,
{
    #[inline]
    fn current_span_len(&self) -> Option<usize> {
        self.input.current_span_len()
    }

    #[inline]
    fn channels(&self) -> rodio::ChannelCount {
        self.input.channels()
    }

    #[inline]
    fn sample_rate(&self) -> rodio::SampleRate {
        self.input.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), RodioSeekError> {
        self.input.try_seek(pos)
    }
}

pub fn wrap_source<S: Source>(
    source: S,
    controls: Arc<Controls>,
    source_tx: Sender<SourceEvent>,
    gain_factors: Arc<TrackGainFactors>,
) -> impl Source {
    ControlledSource::new(source, controls, source_tx, gain_factors).periodic_access(
        SOURCE_UPDATE_INTERVAL,
        ControlledSource::update_control_state,
    )
}
