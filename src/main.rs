use std::fs::File;

fn main() {
    let stream_handle = rodio::OutputStreamBuilder::open_default_stream()
        .expect("Could not open default audio stream");

    loop {
        let music_file = File::open("").unwrap();
        let sink = rodio::play(stream_handle.mixer(), music_file).unwrap();
        sink.sleep_until_end();
    }
}
