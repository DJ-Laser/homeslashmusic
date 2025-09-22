use std::{path::PathBuf, time::Duration};

use crate::{
  InsertPosition, LoopMode, PlaybackState, Request, SeekOffset, TrackId, TrackInstance, Version,
  private::SealedRequest,
};

macro_rules! requests {
  (@doc $($doc:literal)* @def $name:ident ()) => {
    $(#[doc = $doc])*
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct $name;
  };

  (@doc $($doc:literal)* @def $name:ident ( $($field:ty),* )) => {
    $(#[doc = $doc])*
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct $name($(pub $field),*);
  };

  (@doc $($doc:literal)* @def $name:ident { $($t:tt)* } ) => {
    $(#[doc = $doc])*
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct $name{$($t)*}
  };

  (
    $($(#[doc = $doc:literal])* $name:ident $fields:tt -> $response:ty;)*
  ) => {
paste::paste! {
  pub(crate) mod private {
    use std::error::Error;

    use crate::{requests};
    use super::*;

    /// Prefer using `Request.into()` or generics when writing requests
    /// This type is only needed to destinguish between requests when sending them to the server
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub enum QualifiedRequest {
      $(
        $name(super::$name),
      )*
    }

    pub async fn _handle_request<E>(request: QualifiedRequest, handler: &(impl RequestHandler<Error = E> + ?Sized)) -> Result<String, E> {
      let reply_data = match request {
        $(
          QualifiedRequest::$name(request) => {
            crate::server::serialize_response::<super::$name>(handler.[<handle_$name:snake>](request).await?)
          }
        )*
      };

      Ok(reply_data)
    }

    pub trait RequestHandler {
      type Error: Error;

      $(
        fn [<handle_$name:snake>](&self, request: requests::$name) -> impl Future<Output = Result<$response, Self::Error>>;
      )*
    }
  }

  use private::QualifiedRequest;

  $(
    requests!(@doc $($doc)* @def $name $fields);

    impl From<$name> for QualifiedRequest {
      fn from(value: $name) -> Self {
        QualifiedRequest::$name(value)
      }
    }

    impl SealedRequest for $name {}
    impl Request for $name {
      type Response = $response;
    }
  )*
}
};
}

requests! {
  /// Get the hsm_ipc::version() of the server.
  QueryVersion() -> Version;

  QueryPlaybackState() -> PlaybackState;
  Play() -> ();
  Pause() -> ();
  StopPlayback() -> ();
  TogglePlayback() -> ();

  /// Get the metadata for the currently selected track.
  QueryCurrentTrack() -> Option<TrackInstance>;
  /// Get only the track id for the currently selected track.
  QueryCurrentTrackId() -> Option<TrackId>;

  /// Skip to the specified track if it exists.
  /// Returns true if the track exists and was skipped to.
  GoToTrack(TrackId) -> bool;

  /// Skip to the specified track if it exists and starts the track at the specified duration.
  /// Note that if the duration is too large the track will not play and skip to the next one.
  /// Returns true if the track exists and was skipped to.
  GoToTrackAt(TrackId, Duration) -> bool;

  /// Skip to the specified index if it exists.
  /// Returns true if a song at the index exists and was skipped to.
  GoToQueueIndex(usize) -> bool;

  GoToNextTrack() -> ();
  GoToPreviousTrack {
    /// Restarts the track instead of going to the previous track if enough time has passed.
    pub soft: bool,
  } -> ();

  QueryLoopMode() -> LoopMode;
  SetLoopMode(LoopMode) -> ();

  QueryShuffle() -> bool;
  SetShuffle(bool) -> ();

  /// Returns a fraction from 0-1 where 0 is muted and 1 is full volume.
  QueryVolume() -> f32;
  /// Returns a fraction from 0-1 where 0 is muted and 1 is full volume.
  /// Values less than 0 or greater than 1 will be clamped.
  SetVolume(f32) -> ();

  /// Returns the position within the current track.
  QueryPosition() -> Duration;
  /// Seeks within the current track.
  SeekCurrent(SeekOffset) -> ();
  /// Seeks to a fixed position within the current song.
  /// If the provided TrackId is not equal to the current track, the request is discarded as stale.
  /// Returns true the seek was successful, and false if it was discarded.
  SeekTo(TrackId, Duration) -> bool;

  QueryQueueTracks() -> Vec<TrackInstance>;
  QueryQueueTrackIds() -> Vec<TrackId>;

  ClearTracks() -> ();
  LoadTracks(InsertPosition, Vec<PathBuf>) -> Vec<(PathBuf, String)>;
}
