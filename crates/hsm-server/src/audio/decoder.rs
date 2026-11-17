use std::{fs::File, io, path::Path, time::Duration};

use rodio::{
    ChannelCount, Decoder, Sample, SampleRate, Source, decoder::DecoderError,
    source::SeekError as RodioSeekError,
};
use thiserror::Error;

use crate::database::TrackLocation;

#[derive(Debug, Error)]
pub enum LoadTrackError {
    #[error("{0}")]
    Open(#[source] io::Error),

    #[error("{0}")]
    CreateTrackDecoderFailed(#[source] DecoderError),
}

/// A `Source` that decodes `Track`s using symphonia
pub(crate) struct TrackDecoder {
    decoder: Decoder<File>,
}

impl TrackDecoder {
    pub async fn new(location: TrackLocation) -> Result<Self, LoadTrackError> {
        smol::unblock(move || Self::new_sync(location)).await
    }

    fn new_sync(location: TrackLocation) -> Result<Self, LoadTrackError> {
        match location {
            TrackLocation::LocalFile(path) => Self::new_local(&path),
        }
    }

    fn new_local(path: &Path) -> Result<Self, LoadTrackError> {
        let file = File::open(path).map_err(LoadTrackError::Open)?;
        let len = file.metadata().map_err(LoadTrackError::Open)?.len();
        let mut decoder = Decoder::builder().with_data(file).with_byte_len(len);

        if let Some(hint) = path.extension().and_then(|s| s.to_str()) {
            decoder = decoder.with_hint(hint);
        }

        let decoder = decoder
            .build()
            .map_err(LoadTrackError::CreateTrackDecoderFailed)?;

        Ok(TrackDecoder { decoder })
    }
}

impl Iterator for TrackDecoder {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        self.decoder.next()
    }
}

impl Source for TrackDecoder {
    #[inline]
    fn current_span_len(&self) -> Option<usize> {
        self.decoder.current_span_len()
    }

    #[inline]
    fn channels(&self) -> ChannelCount {
        self.decoder.channels()
    }

    #[inline]
    fn sample_rate(&self) -> SampleRate {
        self.decoder.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.decoder.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), RodioSeekError> {
        self.decoder.try_seek(pos)
    }
}
