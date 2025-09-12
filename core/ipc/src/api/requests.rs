use std::{path::PathBuf, time::Duration};

use super::{
  InsertPosition, LoopMode, PlaybackState, Request, SeekPosition, Track, TrackListSnapshot,
  Version, private::SealedRequest,
};

macro_rules! requests {
  (@def $name:ident ()) => {
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct $name;
  };

  (@def $name:ident ( $($field:ty),* )) => {
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct $name($(pub $field),*);
  };

  (@def $name:ident { $($t:tt)* } ) => {
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct $name{$($t)*}
  };

  (
    $($name:ident $fields:tt -> $response:ty;)*
  ) => {
paste::paste! {
  pub(crate) mod private {
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
      type Error: ToString;

      $(
        fn [<handle_$name:snake>](&self, request: requests::$name) -> impl Future<Output = Result<$response, Self::Error>>;
      )*
    }
  }

  use private::QualifiedRequest;

  impl<R: Request> From<R> for QualifiedRequest {
    fn from(value: R) -> Self {
      value.into()
    }
  }

  $(
    requests!(@def $name $fields);

    impl SealedRequest for $name {}
    impl Request for $name {
      type Response = $response;
    }
  )*
}
};
}

requests! {
  QueryVersion() -> Version;

  QueryPlaybackState() -> PlaybackState;
  Play() -> ();
  Pause() -> ();
  StopPlayback() -> ();
  TogglePlayback() -> ();

  QueryCurrentTrack() -> Option<Track>;
  QueryCurrentTrackIndex() -> usize;
  NextTrack() -> ();
  PreviousTrack {
    /// Restarts the track instead of going to the previous track if enough time has passed
    pub soft: bool,
  } -> ();

  QueryLoopMode() -> LoopMode;
  SetLoopMode(LoopMode) -> ();

  QueryShuffle() -> bool;
  SetShuffle(bool) -> ();

  QueryVolume() -> f32;
  SetVolume(f32) -> ();

  QueryPosition() -> Duration;
  Seek(SeekPosition) -> ();

  QueryTrackList() -> TrackListSnapshot;
  ClearTracks() -> ();
  LoadTracks(InsertPosition, Vec<PathBuf>) -> Vec<(PathBuf, String)>;
}
