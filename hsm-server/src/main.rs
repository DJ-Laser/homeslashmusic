fn main() {
  let stream_handle =
    rodio::OutputStreamBuilder::open_default_stream().expect("Could not open default audio stream");

  let ex = smol::Executor::new();

  smol::block_on(ex.run(async move { todo!() }));
}
