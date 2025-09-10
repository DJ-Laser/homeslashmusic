use std::{sync::Arc, time::Duration};

use symphonia::core::{
  audio::{SampleBuffer, SignalSpec},
  codecs::{CODEC_TYPE_NULL, Decoder, DecoderOptions},
  errors::{Error as SymphoniaError, SeekErrorKind as SymphoniaSeekError},
  formats::{FormatReader, SeekMode, SeekTo, SeekedTo},
};

use rodio::{ChannelCount, Sample, SampleRate, Source, source::SeekError as RodioSeekError};

use crate::audio_server::track::{self, LoadTrackError, LoadedTrack};

/// A `Source` that decodes `Track`s using symphonia
pub(crate) struct TrackDecoder {
  decoder: Box<dyn Decoder>,
  current_span_offset: usize,
  format: Box<dyn FormatReader>,
  total_duration: Option<Duration>,
  buffer: SampleBuffer<Sample>,
  spec: SignalSpec,
}

impl TrackDecoder {
  pub async fn new(track: Arc<LoadedTrack>) -> Result<Self, LoadTrackError> {
    smol::unblock(move || Self::new_sync(track)).await
  }

  fn new_sync(track: Arc<LoadedTrack>) -> Result<Self, LoadTrackError> {
    println!("Creating decoder for track {:?}", track.file_path());

    let probed = track::probe_track_sync(track.file_path())?;
    let audio_track = probed
      .format
      .tracks()
      .iter()
      .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
      .ok_or(LoadTrackError::CodecNotSupported)?;

    let decoder = symphonia::default::get_codecs()
      .make(&audio_track.codec_params, &DecoderOptions::default())
      .map_err(|_| LoadTrackError::CodecNotSupported)?;

    let buffer = SampleBuffer::new(0, track.spec);
    Ok(TrackDecoder {
      decoder,
      current_span_offset: 0,
      format: probed.format,
      total_duration: track.inner.total_duration,
      buffer,
      spec: track.spec,
    })
  }

  /// Note span offset must be set after
  fn try_refine_position(&mut self, seek_res: SeekedTo) -> Result<(), RodioSeekError> {
    let Some(time_base) = self.decoder.codec_params().time_base else {
      return Ok(());
    };

    // Calculate the number of samples to skip.
    let mut samples_to_skip =
      (Duration::from(time_base.calc_time(seek_res.required_ts.saturating_sub(seek_res.actual_ts)))
        .as_secs_f32()
        * self.sample_rate() as f32
        * self.channels() as f32)
        .ceil() as usize;

    // Re-align the seek position to the first channel.
    samples_to_skip -= samples_to_skip % self.channels() as usize;

    // Skip ahead to the precise position.
    for _ in 0..samples_to_skip {
      self.next();
    }

    Ok(())
  }
}

impl Iterator for TrackDecoder {
  type Item = Sample;

  fn next(&mut self) -> Option<Self::Item> {
    if self.current_span_offset >= self.buffer.len() {
      let decoded = loop {
        let packet = self.format.next_packet().ok()?;
        let decoded = match self.decoder.decode(&packet) {
          Ok(decoded) => decoded,
          Err(SymphoniaError::DecodeError(_)) => {
            // Skip over packets that cannot be decoded. This ensures the iterator
            // continues processing subsequent packets instead of terminating due to
            // non-critical decode errors.
            continue;
          }
          Err(_) => return None,
        };

        // Loop until we get a packet with audio frames. This is necessary because some
        // formats can have packets with only metadata, particularly when rewinding, in
        // which case the iterator would otherwise end with `None`.
        // Note: checking `decoded.frames()` is more reliable than `packet.dur()`, which
        // can resturn non-zero durations for packets without audio frames.
        if decoded.frames() > 0 {
          break decoded;
        }
      };

      self.buffer = SampleBuffer::new(decoded.capacity() as u64, self.spec);
      self.buffer.copy_interleaved_ref(decoded);
      self.current_span_offset = 0;
    }

    let sample = *self.buffer.samples().get(self.current_span_offset)?;
    self.current_span_offset += 1;

    Some(sample)
  }
}

impl Source for TrackDecoder {
  #[inline]
  fn current_span_len(&self) -> Option<usize> {
    Some(self.buffer.len())
  }

  #[inline]
  fn channels(&self) -> ChannelCount {
    self.spec.channels.count() as ChannelCount
  }

  #[inline]
  fn sample_rate(&self) -> SampleRate {
    self.spec.rate
  }

  #[inline]
  fn total_duration(&self) -> Option<Duration> {
    self.total_duration
  }

  fn try_seek(&mut self, pos: Duration) -> Result<(), RodioSeekError> {
    // Seeking should be "saturating", meaning: target positions beyond the end of the stream
    // are clamped to the end.
    let mut target = pos;
    if let Some(total_duration) = self.total_duration {
      if target > total_duration {
        target = total_duration;
      }
    }

    // Remember the current channel, so we can restore it after seeking.
    let active_channel = self.current_span_offset % self.channels() as usize;

    let seek_res = match self.format.seek(
      SeekMode::Accurate,
      SeekTo::Time {
        time: target.into(),
        track_id: None,
      },
    ) {
      Err(SymphoniaError::SeekError(SymphoniaSeekError::ForwardOnly)) => {
        return Err(RodioSeekError::NotSupported {
          underlying_source: std::any::type_name::<Self>(),
        });
      }
      other => other.map_err(|error| RodioSeekError::Other(Box::new(error))),
    }?;

    // Seeking is a demuxer operation without the decoder knowing about it,
    // so we need to reset the decoder to make sure it's in sync and prevent
    // audio glitches.
    self.decoder.reset();

    // Force the iterator to decode the next packet.
    self.current_span_offset = usize::MAX;

    // Symphonia does not seek to the exact position, it seeks to the closest keyframe.
    // If accurate seeking is required, fast-forward to the exact position.
    self.try_refine_position(seek_res)?;

    // After seeking, we are at the beginning of an inter-sample frame, i.e. the first
    // channel. We need to advance the iterator to the right channel.
    for _ in 0..active_channel {
      self.next();
    }

    Ok(())
  }
}
