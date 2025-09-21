use std::{fs::File, path::PathBuf, time::Duration};

use rodio::{
  ChannelCount, Decoder, Sample, SampleRate, Source, source::SeekError as RodioSeekError,
};

use crate::audio_server::track::LoadTrackError;

/// A `Source` that decodes `Track`s using symphonia
pub(crate) struct TrackDecoder {
  decoder: Decoder<File>,
}

impl TrackDecoder {
  pub async fn new(path: PathBuf) -> Result<Self, LoadTrackError> {
    smol::unblock(move || Self::new_sync(path)).await
  }

  fn new_sync(path: PathBuf) -> Result<Self, LoadTrackError> {
    let file = File::open(&path).map_err(LoadTrackError::OpenFailed)?;
    let len = file.metadata().map_err(LoadTrackError::OpenFailed)?.len();
    let mut decoder = Decoder::builder().with_data(file).with_byte_len(len);

    if let Some(hint) = path.extension().and_then(|s| s.to_str()) {
      decoder = decoder.with_hint(hint);
    }

    println!("qa");
    let decoder = decoder
      .build()
      .map_err(LoadTrackError::CreateTrackDecoderFailed)?;
    println!("qa2");

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
