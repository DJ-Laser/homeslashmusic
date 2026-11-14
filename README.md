# homeslashmusic Music Database

### MusicBrainz enabled music database

**Note: This program is linux-only due to using both unix sockets and the MPRIS d-bus interface. If you are on windows, this program will not compile, and I will not be fixing that.**

## Database Structure

The homeslashmusic database is built to reliably handle hundreds of audio tracks, from local files to streaming service urls. To reliably index and dedupicate songs and albums, uses MusicBrainz metadata and separately indexes all audio sources, even if they are for the same song. The database has six entity types that work together to provide the Who, What, and Where for your music library.

### Tracks

A Track is a single source of audio data. They are usually file paths, but can also be streaming service ids. Each Track may correspond to one song and/or one TrackGroup. A Track is identified by a TrackLocation, which determines where to load the audio data from and may correspond to a url.

### TrackGroups

A TrackGroup is a collection of Tracks from one source. TrackGroups are created from the album tags on local files, and album ids from streaming services. Each TrackGroup may correspond to one Album. A TrackGroup is identified by a TrackGroupLocation, and may correspond to a url.

### Songs

A Song is a grouping entity for Tracks. All Tracks that have data for one "song" (in the general sense) correspond to the same song. A song may correspond to one album. A Song is identified by one MusicBrainz Recording id.

### Albums

An Album is a grouping entity for TrackGroups. All TrackGroups that collect tracks for the same "album" (in the general sense) correspond to the same Album. An Album is identified by one MusicBrainz ReleaseGroup id.

### Artists

An Artist represents the author of various Tracks, TrackGroups, Songs, and ALbums. Each of the aforementioned entities can correspond to multiple artists. An Artist may be identified by one MusicBrainz Artist id. An Artist may also be identified by a list of TrackGroupLocations, which may correspnd to urls.

### Playlists

A Playlist is a collection of unrelated entities that should be played in sequence. It may include Tracks, TrackGroups, Songs, and Albums.

### Loading/Playback

While Tracks and TrackGroups define the location of audio data, users may also want to load audio by Songs and Albums. In this case, the list of Tracks/TrackGroups associated with The Song/Album is used, and the highest priority TrackLocation/TrackGroupLocation determines which Track/TrackGroup to load.

## Installation

Run `nix shell github:dj-laser/homeslashmusic` to try the program without permanantly installing.

## Usage

Run `hsm-server <filepath>` to load tracks into the database. You can use a path to a single audio file, or a directory, which will be searched recursively. There is no persistence currenlty, so terminating the program will delete any added entities. There is currently no support for loading playlist files.

Audio files must be tagged with MusicBrainz ids to fetch Song, Album, and Artist information. See [MusicBrainz Picard](https://picard.musicbrainz.org/) for a free MusicBrainz-aware tagging program. Audio files without MusicBrainz tags will load Tracks and TrackGroups, but not Songs, Artists, or Albums.

Loading local tracks is very fast, but fetching metadata from MusicBrainz is limited to one request per second. For each audio file, this program fetches it's Recording, ReleaseGroup, and any Artists in the metadata. If you don't want to wait, try running the program on a small number of tracks.

Example output from four music files. Because "Who Are You?" has dj-mix relationships on MusicBrainz, all the mixed songs it contained were added to the database.

```
##### TRACKS #####

Unknown Track - Unknown Artist
Unknown Track - Unknown Artist
Who Are You? – Breakcore Mix to Dissociate - MabelSama
LFAV! PLAYS THE FIRST 161 SONGS SHE CAN THINK OF @ HI SCORES HOUSE - leastfavorite!

##### TRACK GROUPS #####

Who Are You? – Breakcore Mix to Dissociate - MabelSama (1 tracks)
LFAV! PLAYS THE FIRST 161 SONGS SHE CAN THINK OF @ HI SCORES HOUSE - leastfavorite! (1 tracks)

##### SONGS #####

Who Are You? – Breakcore Mix to Dissociate - YouTubeZombies, (10 mixed songs)
LFAV! PLAYS THE FIRST 161 SONGS SHE CAN THINK OF @ HI SCORES HOUSE - leastfavorite!, (0 mixed songs)
angelic - nihigo w/ nunashi, (0 mixed songs)
Altered Reality - DreamWeaver ft. Botanical Anomaly, (0 mixed songs)
Mr. Kill Myself - Sewerslvt, (0 mixed songs)
temporary - Lafew, (0 mixed songs)
NoLoveWorld - Gutterpink, (0 mixed songs)
TERMINAL 18 - LilyNiku feat. Whivvy, (0 mixed songs)
卒業ですね (Sewerslvt remix) - AZALEA, (0 mixed songs)
Get Your Wish (Sewerslvt remix) - Porter Robinson, (0 mixed songs)
Newlove - Sewerslvt, (0 mixed songs)
I Just Want to Exist - Empty Blue, (0 mixed songs)

##### ALBUMS #####

Who Are You? – Breakcore Mix to Dissociate - YouTubeZombies, (1 track groups)
LFAV! PLAYS THE FIRST 161 SONGS SHE CAN THINK OF @ HI SCORES HOUSE - leastfavorite!, (1 track groups)

##### ARTISTS #####

MabelSama, (1 songs and 1 albums)
leastfavorite!, (1 songs and 1 albums)
nihigo, (1 songs and 0 albums)
nunashi, (1 songs and 0 albums)
DreamWeaver, (1 songs and 0 albums)
Botanical Anomaly, (1 songs and 0 albums)
Sewerslvt, (2 songs and 0 albums)
lafew, (1 songs and 0 albums)
Gutterpink, (1 songs and 0 albums)
Noelle Stockwood, (1 songs and 0 albums)
Whivvy, (1 songs and 0 albums)
AZALEA, (1 songs and 0 albums)
Porter Robinson, (1 songs and 0 albums)
Empty Blue, (1 songs and 0 albums)
```

# Old Instrustions for homeslashmusic

### minimal client-server audio player

**Note: This program is currently setup to demo the music database only. Audio server functionality will not work. The old readme is kept for when I eventually merge the two features.**

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
