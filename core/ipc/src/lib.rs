use git_version::git_version;
use std::{env, sync::OnceLock};

pub use api::*;
mod api;

pub fn version() -> Version {
  const fn parse_version_num(number_string: &str) -> usize {
    match usize::from_str_radix(number_string, 10) {
      Ok(num) => num,
      Err(_) => panic!("Failed to parse version number"),
    }
  }

  const MAJOR: usize = parse_version_num(env!("CARGO_PKG_VERSION_MAJOR"));
  const MINOR: usize = parse_version_num(env!("CARGO_PKG_VERSION_MINOR"));
  const PATCH: usize = parse_version_num(env!("CARGO_PKG_VERSION_PATCH"));
  const COMMIT: &str = git_version!(fallback = "unknown commit");

  Version {
    major: MAJOR,
    minor: MINOR,
    patch: PATCH,
    commit: COMMIT.into(),
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
