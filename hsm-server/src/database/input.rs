use std::time::Duration;

use crate::database::{AudioLocation, MixTracks, MusicBrainzId};

/// Insert
#[derive(Debug)]
pub struct TrackInputWithMbid {
    audio_location: AudioLocation,
    metadata_location: MusicBrainzId,

    name: String,
    image: Option<AudioLocation>,
    artists: Vec<(MusicBrainzId, String)>,
    album: Option<(MusicBrainzId, u64)>,
    genres: Vec<String>,
    date: Option<String>,
    length: Duration,
}

// data flow:
// song file with track metadata
// -  track has musicbrainz track id tag
// -  -  musicbrainz metadata is fetched asynchronously
// -  artist/albumartist tag on file has artists that need to be in the db before track can be added
// -  -  musicbrainz artist tags create new artist entries in db
// -  -  musicbrainz metadata is fetched asynchronously
// -  contains album tag, album needs to be added to db
// -  -  musicbrainz album tag creates new db entry
// -  -  musicbrainz metadata is fetched asynchronously
// -  database ids for album and artist are used to make database entry for track
// eventually musicbrainz queries finish, db entries are updated with that info
//
// if no musicbrainz tag is present, find it with accoustid
// if track is not in accoustid database use file metadata
