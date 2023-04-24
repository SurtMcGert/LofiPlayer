use rodio::source::Source;
use rodio::{Decoder, OutputStream, Sink};
use std::iter::Iterator;
use std::path::Path;
use std::time::Duration;
use std::{fs, thread};
use std::{fs::File, io::BufReader};
use {std::sync::mpsc, tray_item::TrayItem};

enum Message {
    Quit,
}

fn main() {
    let lofiDirectory = "tracks/lofiMusic";
    let handle = thread::spawn(|| {
        // Do something in the new thread.
        let backgroundDirectory = "tracks/backgroundSound";
        loop {
            let f = getRndTrack(&backgroundDirectory);
            playTrack(f, 0.1);
        }
    });

    loop {
        let f = getRndTrack(&lofiDirectory);
        playTrack(f, 0.25);
    }
}

fn getRndTrack(directory: &str) -> File {
    let mut rng = rand::thread_rng();
    let files = fs::read_dir(directory).unwrap();
    let file;

    let itr = rand::seq::IteratorRandom::choose(files, &mut rand::thread_rng());
    let binding = itr.unwrap().unwrap().path();
    println!("{}", binding.to_str().unwrap());
    file = binding.to_str().unwrap();

    return File::open(file).unwrap();
}

fn playTrack(file: File, vol: f32) {
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    // Load a sound from a file, using a path relative to Cargo.toml
    // let file = BufReader::new(File::open("music/test.mp3").unwrap());
    // Decode that sound file into a source
    let mut source = Decoder::new(file).unwrap();
    sink.set_volume(vol);
    sink.append(source);

    // The sound plays in a separate thread. This call will block the current thread until the sink
    // has finished playing all its queued sounds.
    sink.sleep_until_end();
    sink.clear();
}
