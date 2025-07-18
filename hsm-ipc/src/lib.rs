use git_version::git_version;
use std::{env, sync::OnceLock};

pub use api::*;
mod api;

pub fn version() -> String {
  const MAJOR: &str = env!("CARGO_PKG_VERSION_MAJOR");
  const MINOR: &str = env!("CARGO_PKG_VERSION_MINOR");
  const PATCH: &str = env!("CARGO_PKG_VERSION_PATCH");

  let commit = git_version!(fallback = "unknown commit");

  if PATCH == "0" {
    format!("{MAJOR}.{MINOR:0>2} ({commit})")
  } else {
    format!("{MAJOR}.{MINOR:0>2}.{PATCH} ({commit})")
  }
}

fn read_socket_path() -> String {
  let runtime_path = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
    let uid = rustix::process::getuid();
    format!("/run/user/{}", uid.as_raw())
  });

  format!("{runtime_path}/homeslashmusic.sock")
}

pub fn socket_path() -> &'static str {
  static PATH: OnceLock<String> = OnceLock::new();
  PATH.get_or_init(read_socket_path)
}
