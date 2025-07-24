# homeslashmusic

### minimal client-server audio player

**Note: This program is linux-only due to using both unix sockets and the MPRIS d-bus interface. If you are on windows, this program will not compile, and I will not be fixing that.**

## Installation

Run `nix shell github:dj-laser/homeslashmusic` to try the program without permanantly installing.

Run `hsm-server` to start the audio server.
The audio server is what actually plays audio. It listens for ipc messages over unix socket and for [MIPRS](https://specifications.freedesktop.org/mpris-spec/latest/index.html) messages over d-bus.

Run `hsm` to control the backend by sending messages for example `hsm play-pause`.
Use `hsm help` to see the available options

For permanant instalation, add `github:dj-laser/homeslashmusic` as a flake input.

This flake exports a `packages.x86_64-linux.homeslashmusic`, or you can use the `overlays.default` to add `homeslashmusic` to `pkgs`.

Finally, configure `hsm-server` to run on login.

This could be done a few ways, such as a systemd service or through the window manager.

Example using [`niri-flake`](https://https://github.com/sodiboo/niri-flake)

```nix
programs.niri = {
  settings.spawn-at-startup = [
    # Assuming homeslashmusic has been added to your packages
    {command = ["hsm-server"];}

    # More explicit way to define it
    # {command = ["${pkgs.homeslashmusic}/bin/hsm-server"];}
  ];
};
```

## Usage

The `hsm-server` program runs the audio server. Once it is running, you may use the `hsm` program to control playback.
Run `hsm help` to see available options.

`hsm-server` also implements the [MIPRS](https://specifications.freedesktop.org/mpris-spec/latest/index.html) d-bus interface, so it is possible to control it using programs such as `playerctl`.

## Configuration

Niether `hsm-server` nor `hsm` currently have any kind of config files. Run `hsm help` to see available options for controling playback such as looping.

## Technologies used

- **nix (❤️):** provides a reproducible dev environment and package build
