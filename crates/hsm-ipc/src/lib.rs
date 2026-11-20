mod api;

use directories::ProjectDirs;
use git_version::git_version;
use std::{
    env,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub use api::*;

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

fn read_socket_path() -> PathBuf {
    let runtime_path = ProjectDirs::from("", "", "homeslashmusic")
        .and_then(|dirs| dirs.runtime_dir().map(PathBuf::from))
        .unwrap_or_else(|| {
            let uid = rustix::process::getuid();
            PathBuf::from(format!("/run/user/{}/homeslashmusic", uid.as_raw()))
        });

    if cfg!(debug_assertions) {
        runtime_path.join("homeslashmusic-debug.sock")
    } else {
        runtime_path.join("homeslashmusic.sock")
    }
}

pub fn socket_path() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(read_socket_path)
}
