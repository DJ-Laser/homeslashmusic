use std::{path::PathBuf, time::Duration};

use crate::{
    InsertItem, InsertPosition, LoopMode, PlaybackState, QueuedTrack, QueuedTrackId, SearchResult,
    SeekOffset, Version, make_requests,
};

make_requests! {
  /// Get the `hsm_ipc::version()` of the server.
  GetVersion() -> Version;

  // ####################
  // ## Queue/Playback ##
  // ####################

  GetPlaybackState() -> PlaybackState;
  Play() -> ();
  Pause() -> ();
  StopPlayback() -> ();
  TogglePlayback() -> ();

  /// Get the metadata for the current track.
  GetCurrentQueuedTrack() -> Option<QueuedTrack>;
  /// Get only the queued track id for the current track.
  GetCurrentQueuedTrackId() -> Option<QueuedTrackId>;

  /// Skip to the specified track if it exists.
  /// Returns true if the track exists and was skipped to.
  GoToTrack(QueuedTrackId) -> bool;
  /// Skip to the specified index if it exists.
  /// Returns true if a song at the index exists and was skipped to.
  GoToQueueIndex(usize) -> bool;

  GoToNextTrack() -> ();
  GoToPreviousTrack() -> ();

  GetLoopMode() -> LoopMode;
  SetLoopMode(LoopMode) -> ();

  GetShuffle() -> bool;
  SetShuffle(bool) -> ();

  /// Returns a fraction from 0-1 where 0 is muted and 1 is full volume.
  GetVolume() -> f32;
  /// Accepts a fraction from 0-1 where 0 is muted and 1 is full volume.
  /// Values less than 0 or greater than 1 will be clamped.
  SetVolume(f32) -> ();

  /// Returns the position within the current track.
  GetPosition() -> Duration;
  /// Seeks within the current track.
  SeekCurrent(SeekOffset) -> ();
  /// Seeks to a fixed position within the current song.
  /// If the provided `TrackId` is not equal to the current track, the request is discarded as stale.
  /// Returns true the seek was successful, and false if it was discarded.
  SeekTo(QueuedTrackId, Duration) -> bool;

  /// Get the metadata for the tracks in the queue.
  GetQueuedTracks() -> Vec<QueuedTrack>;
  /// Get only the queued track id for the tracks in the queue.
  GetQueuedTrackIds() -> Vec<QueuedTrackId>;

  /// Remove all tracks from the queue and stop playback.
  ClearQueue() -> ();

  /// Load tracks from the database into the queue.
  /// The id of the first queued track will be returned.
  LoadTracks(InsertItem, InsertPosition) -> QueuedTrackId;

  // ##############
  // ## Dataabse ##
  // ##############

  /// Start a full database update
  UpdateDatabase(PathBuf) -> ();

  Search(String) -> Option<SearchResult>;
}
