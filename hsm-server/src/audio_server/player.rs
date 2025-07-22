use std::{
  fs::File as SyncFile,
  io::{self, BufReader as SyncBufReader},
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

use atomic_enum::atomic_enum;
use rodio::{
  Decoder, Source,
  mixer::Mixer,
  queue::{SourcesQueueInput, SourcesQueueOutput, queue},
  source,
};
use smol::{
  channel::{self, Receiver, Sender},
  lock::Mutex,
};

enum SourceEvent {
  Finsihed,
}

#[atomic_enum]
enum PlaybackState {
  Playing,
  Paused,
  Stopped,
}

struct Controls {
  pub playback_state: AtomicPlaybackState,
  pub to_skip: AtomicUsize,
  pub volume: Mutex<f32>,
}

impl Controls {
  pub fn new() -> Self {
    Self {
      playback_state: AtomicPlaybackState::new(PlaybackState::Stopped),
      to_skip: AtomicUsize::new(0),
      volume: Mutex::new(1.0),
    }
  }
}

pub struct Player {
  track_queue: Arc<SourcesQueueInput>,
  controls: Arc<Controls>,
  source_tx: Sender<SourceEvent>,
  source_rx: Receiver<SourceEvent>,
}

impl Player {
  const SOURCE_UPDATE_INTERVAL: Duration = Duration::from_millis(5);

  pub fn connect_new(mixer: &Mixer) -> Self {
    let (player, source) = Self::new();
    mixer.add(source);
    player
  }

  pub fn new() -> (Self, SourcesQueueOutput) {
    let (queue_in, queue_out) = queue(true);
    let (source_tx, source_rx) = channel::unbounded();

    (
      Self {
        track_queue: queue_in,
        controls: Arc::new(Controls::new()),
        source_tx,
        source_rx,
      },
      queue_out,
    )
  }

  fn wrap_source<S: Source + Send + 'static>(&self, source: S) -> impl Source + Send + 'static {
    let controls = self.controls.clone();
    let source_tx = self.source_tx.clone();

    let source = source
      .track_position()
      .amplify(1.0)
      .pausable(false)
      .skippable()
      .periodic_access(Self::SOURCE_UPDATE_INTERVAL, move |skippable| {
        {
          let to_skip = controls.to_skip.load(Ordering::Acquire);
          if to_skip > 0 {
            skippable.skip();
            controls.to_skip.store(to_skip - 1, Ordering::Release);
            return;
          }
        }
        let pauseable = skippable.inner_mut();
        pauseable.set_paused(matches!(
          controls.playback_state.load(Ordering::Relaxed),
          PlaybackState::Paused
        ));
        let volume_controlled = pauseable.inner_mut();
        volume_controlled.set_factor(*controls.volume.lock_blocking());
      });
    source::from_iter([
      Box::new(source) as Box<dyn Source + Send>,
      Box::new(source::EmptyCallback::new(Box::new(move || {
        source_tx.try_send(SourceEvent::Finsihed).unwrap();
      }))) as Box<dyn Source + Send>,
    ])
  }

  async fn load_file<P: AsRef<Path>>(&self, path: P) -> Result<impl Source + 'static, io::Error> {
    let path = path.as_ref().to_owned();
    let (path, file, len) = smol::unblock(|| {
      let file = SyncFile::open(&path)?;
      let len = file.metadata()?.len();

      Ok::<_, io::Error>((path, file, len))
    })
    .await?;

    let mut builder = Decoder::builder()
      .with_data(SyncBufReader::new(file))
      .with_byte_len(len);

    if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
      builder = builder.with_hint(extension);
    }

    let decoder = builder.build().map_err(io::Error::other)?;
    Ok(self.wrap_source(decoder))
  }

  pub async fn run(&self) -> Result<(), io::Error> {
    let add_source = async || {
      let source = self
        .load_file("/home/dj_laser/Music/ASGORE - trap remix w_ mythic apex.mp3")
        .await
        .unwrap();
      self.track_queue.append(source);
    };

    add_source().await;

    loop {
      let event = match self.source_rx.recv().await {
        Ok(event) => event,
        Err(_) => unreachable!("Source event channel should never close while in use"),
      };

      match event {
        SourceEvent::Finsihed => add_source().await,
      }
    }
  }

  pub fn set_playing(&self, playing: bool) {
    if playing {
      self
        .controls
        .playback_state
        .store(PlaybackState::Playing, Ordering::Relaxed);
    } else {
      // Don't un-stop playback on pause
      let _ = self.controls.playback_state.compare_exchange(
        PlaybackState::Playing,
        PlaybackState::Paused,
        Ordering::Relaxed,
        Ordering::Relaxed,
      );
    }
  }

  pub fn toggle_playback(&self) {
    let current_state = self.controls.playback_state.load(Ordering::Relaxed);
    let new_state = match current_state {
      PlaybackState::Paused | PlaybackState::Stopped => PlaybackState::Playing,
      PlaybackState::Playing => PlaybackState::Paused,
    };
    self
      .controls
      .playback_state
      .store(new_state, Ordering::Relaxed);
  }
}
