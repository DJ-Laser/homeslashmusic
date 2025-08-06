use std::path::PathBuf;

use super::{InsertPosition, LoopMode, Request, SeekPosition, private::SealedRequest, responses};

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
    use crate::{requests, Reply};

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub enum QualifiedRequest {
      $(
        $name(super::$name),
      )*
    }

    pub trait RequestHandler {
      $(
        fn [<handle_ $name:snake>](&self, request: requests::$name) -> impl Future<Output = Reply<requests::$name>>;
      )*
    }

    pub async fn handle_request(request_data: &str, handler: &impl RequestHandler) -> String {
      let request = match serde_json::from_str(request_data) {
        Ok(request) => request,
        Err(error) => {
          println!("{}", &error);
          return crate::server::serialize_error(error.to_string());
        }
      };

      match request {
        $(
          QualifiedRequest::$name(request) => {
            crate::server::serialize_reply::<super::$name>(&handler.[<handle_ $name:snake>](request).await)
          }
        )*
      }
    }
  }

  $(
    requests!(@def $name $fields);

    impl SealedRequest for $name {
      fn qualified_request(self) -> private::QualifiedRequest {
        private::QualifiedRequest::$name(self)
      }
    }
    impl Request for $name {
      type Response = $response;
    }
  )*
}
};
}

requests! {
  Version() -> responses::Version;

  Play() -> ();
  Pause() -> ();
  StopPlayback() -> ();
  TogglePlayback() -> ();

  SetVolume(f32) -> ();

  SetLoopMode(LoopMode) -> ();

  Seek(SeekPosition) -> ();

  ClearTracks() -> ();
  LoadTracks(InsertPosition, Vec<PathBuf>) -> Vec<(PathBuf, String)>;
}
