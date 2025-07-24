{
  lib,
  fetchFromGitHub,
  rustPlatform,
  pkg-config,
  alsa-lib,
}:
rustPlatform.buildRustPackage rec {
  pname = "homeslashmusic";
  version = "0.1.0";
  src = ../.;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  nativeBuildInputs = [pkg-config];
  buildInputs = [alsa-lib];

  preBuild = ''
    export HSM_COMPLETION_OUT_DIR=$out/share/bash-completion/completions
    mkdir -p $HSM_COMPLETION_OUT_DIR
  '';

  postFixup = ''
    patchelf --add-rpath ${lib.makeLibraryPath [alsa-lib]} $out/bin/hsm-server
  '';

  meta = {
    mainProgram = "hsm";
    platforms = lib.platforms.linux;
  };
}
