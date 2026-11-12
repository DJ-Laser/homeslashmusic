### PLANS:

Database is a persistent store for all music-related metadata
Stores 5 things:

- Track: name, artists, album, other metadata
- Album: name, artists, other metadata
- Artist: name, other metadata
- Playlist: name, tracks
- Mix: name, tracks + (optional)timestamps

A track is a single song, written by some people and maybe in an album

An album is a collection of tracks, written by someone and released as a unit.

An artist is a person who makes music. This could be tracks, albums, or mixes.

A playlist a colelction of tracks, not released as a unit, and without an artist. Playlists can contain mixes, albums, and other playlists

A mix is like a playlist, but is a single file with (or without) timestamps to identify the current track. It plays the entire file through instead of playing many tracks. Think spotify playlist (playlist) vs dj set (mix) A mix can have nice fades and effects and stuff, while playlists just hard cut to the next song, though playlists can be shuffled while mixes can't. If a mix has timestamps, prev/next can navigate inside the mix if desired and more info can be shown to "now playing" status widgets.

The database should be held in memory for fast access, and therefore be small (<100mb if possible) It must also be serializable to a file for persistence. When hsm-server starts up, it will load the file, then update it's database (scan files, connect to streaming services, etc...)

### Database plugins:

2 types: source plugins and metadata plugins

Source plugins find what tracks are available to listen to. This could be files on your pc, favorited songs from a streaming service, or idk what a third one would be, just a different streaming service perhaps. They can return metadata about the tracks they find, but ultimitely, theur goal is just to find tracks.

Metadata plugins are the opposite. They don't do anything, until tracks are found. Once tracks are found, they are in charge of finding the propper metadata for the tracks, artists, etc... that the source plugins gathered. It is expected that these plugins will need an internet connection, since people aren't going to download the entire MusicBrainz (or similar) database. Metadata gathered by these plugins can be track names, album art, artist image/bio, etc...

In general, during database construction, a working internet connection is assumed. After the database is built, local files can still be searched, and online songs can be as well, though only local songs can be played offline. Offline listening is planned to be supported as a first-class feature, but database contruction while offline is seen as an edge case.

### Storage

Right now this all sounds like a huge spiderweb of information, sources, and dependencies. It kind of is, so I need a clean solution for track identification and deduplication. Unlike before, I can't simply id tracks by their file path, since I have online tracks to deal with.

basically, I will id tracks by their metadata. tracks with the same name by the same artist and in the same album will be considered the same. This does mean that users will need to properly set their file metadata, but this is needed anyway for proper library organization, so it shouldn't be a big issue. Also, when I integrate downloads from web sources, it will automatically set the metadata for you.

The metadata gathered by the source plugins will be used by the metadata plugins to find the cannonical metadata for the resource, defaulting to the source plugin's info. Then it will be instered into the database. If it already exists in the database, the metadata will not be updated (since it already has the metadata from the highest priority metadata plugin), but a new location will be added for where to access the resource.

Locations are the way to handle deduplicating tracks when multiple source plugins find a track. While the track's cannonical metadata from the metadata plugin is it's only entry in the database, that entry may contain multiple locations for where to get the track audio. The location from the highest priority source plugin will be used.

### Metadata sources:

Properly tagged files will have a musicbrainz id associated with them. Audio from streaming services can be reverse-searched by it's url. If neither option works, an accoustid fingerprint can be taken and searched in the accoustid database. MusicBrainz is the favored source of metadata because it has like all the metadata ever in a central place.

Musicbrainz will be the only metadata plugin, because supporting other ones is gonna be too much work. Also, you should just properly tag your music anyway. The one thing I may do not on musicbrainz is artist photos, descriptions, and song lyrics if I add that
