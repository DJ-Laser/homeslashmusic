use smol::channel::{self, Receiver};

pub struct CtrlCHandler {
  reciever: Receiver<()>,
}

impl CtrlCHandler {
  pub fn init() -> Self {
    let (sender, reciever) = channel::bounded(100);
    ctrlc::set_handler(move || {
      sender.try_send(()).ok();
    })
    .unwrap();

    Self { reciever }
  }

  pub async fn wait_for_ctrlc(&self) {
    self.reciever.recv().await.unwrap();
  }
}
