use std::io;

use async_signal::{Signal, Signals};
use smol::stream::StreamExt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignalHandlerError {
  #[error("Failed to register signal handlers: {0}")]
  FailedToRegisterSignalHandlers(io::Error),
}

pub struct SignalHandler {
  signals: Signals,
}

impl SignalHandler {
  pub fn init() -> Result<Self, SignalHandlerError> {
    Ok(Self {
      signals: Signals::new([Signal::Term, Signal::Quit, Signal::Int])
        .map_err(SignalHandlerError::FailedToRegisterSignalHandlers)?,
    })
  }

  pub async fn wait_for_quit(&mut self) {
    while let Some(signal) = self.signals.next().await {
      let Ok(signal) = signal else {
        return;
      };

      if matches!(signal, Signal::Term | Signal::Quit | Signal::Int) {
        return;
      };
    }

    unreachable!("Iterating over Signals should never return None")
  }
}
