use std::time::Duration;

use crate::{LoopMode, PlaybackState};

/// An event than can be sent from the serverasynchronously at any time.
#[derive(Debug, Clone)]
pub enum Event {
    PlaybackStateChanged(PlaybackState),
    LoopModeChanged(LoopMode),
    ShuffleChanged(bool),
    VolumeChanged(f32),
    Seeked(Duration),
}
