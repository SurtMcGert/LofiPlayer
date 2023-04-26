#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::{
    api::shell::open, AppHandle, CustomMenuItem, Manager, SystemTray, SystemTrayEvent,
    SystemTrayMenu, SystemTrayMenuItem, SystemTraySubmenu,
};

use rodio::source::Source;
use rodio::{Decoder, OutputStream, Sink};
use std::iter::Iterator;
use std::path::Path;
use std::sync::mpsc;
// use std::sync::mpsc::{self, TryRecvError};
use crossbeam_channel::{tick, unbounded, Receiver, Sender};
use std::time::{Duration, Instant};
use std::{fs, thread};
use std::{fs::File, io::BufReader};

static mut MUSIC_THREAD: Option<Sender<String>> = None;
static mut PLAYING: bool = true;

fn main() {
    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("quit".to_string(), "Quit"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("playPause-toggle".to_string(), "Pause"));

    let tray = SystemTray::new().with_menu(tray_menu);

    //create a thread to run the music
    unsafe { MUSIC_THREAD = Option::from(createMusicThread()) };

    tauri::Builder::default()
        .system_tray(tray)
        .on_system_tray_event(on_system_tray_event)
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
            }
            _ => {}
        });
}

fn createMusicThread() -> Sender<String> {
    println!("creating music thread");
    let (tx, rx): (Sender<String>, Receiver<String>) = unbounded();
    let handle = thread::spawn(move || {
        let lofiDirectory = "tracks/lofiMusic"; //directory for lofi music
        let backgroundDirectory = "tracks/backgroundSound"; //direcory for background music

        //set up the background music
        let mut backgroundFile: File;
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let backgroundSink = Sink::try_new(&stream_handle).unwrap();
        backgroundSink.set_volume(0.15);

        //set up the lofi music
        let mut lofiFile: File;
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let lofiSink = Sink::try_new(&stream_handle).unwrap();
        lofiSink.set_volume(0.25);

        let ticker: Receiver<Instant> = tick(std::time::Duration::from_millis(500));
        loop {
            crossbeam_channel::select! {
                recv(ticker) -> _ => {
            match rx.try_recv() {
                Err(_e) => {}
                Ok(msg) => {
                    match msg.as_str() {
                        "PLAY" => {
                            println!("playing");
                            backgroundSink.play();
                            lofiSink.play();
                            // continue;
                        }
                        "PAUSE" => {
                            println!("pausing");
                            backgroundSink.pause();
                            lofiSink.pause();
                            // continue;
                        }
                        _ => {}
                    }
                    // continue;
                }
                _ => {}
            }
                }
            }
            //play the background music
            if (backgroundSink.empty()) {
                backgroundFile = getRndTrack(&backgroundDirectory);
                playTrack(&backgroundSink, backgroundFile);
            }
            //play the lofi music
            if (lofiSink.empty()) {
                lofiFile = getRndTrack(&lofiDirectory);
                playTrack(&lofiSink, lofiFile);
            }
        }

        println!("somehow broke out of the loop");
    });
    return tx;
}

fn on_system_tray_event(app: &AppHandle, event: SystemTrayEvent) {
    match event {
        SystemTrayEvent::MenuItemClick { id, .. } => {
            let item_handle = app.tray_handle().get_item(&id);
            dbg!(&id);
            match id.as_str() {
                "playPause-toggle" => match unsafe { PLAYING } {
                    true => {
                        unsafe { (&MUSIC_THREAD.as_ref()).unwrap().send("PAUSE".to_string()) };
                        item_handle.set_title("Play").unwrap();
                        unsafe { PLAYING = false };
                    }
                    false => {
                        unsafe { (&MUSIC_THREAD.as_ref()).unwrap().send("PLAY".to_string()) };
                        item_handle.set_title("Pause").unwrap();
                        unsafe { PLAYING = true };
                    }
                    _ => {}
                },
                "quit" => app.exit(0),
                _ => {}
            }
        }
        _ => {}
    }
}

//function to get a random track
fn getRndTrack(directory: &str) -> File {
    let mut rng = rand::thread_rng();
    let files = fs::read_dir(directory).unwrap();
    let file;

    let itr = rand::seq::IteratorRandom::choose(files, &mut rand::thread_rng());
    let binding = itr.unwrap().unwrap().path();
    println!("got track: {}", binding.to_str().unwrap());
    file = binding.to_str().unwrap();

    return File::open(file).unwrap();
}

//function to play a track
fn playTrack(sink: &Sink, file: File) {
    println!("playing track");
    // Decode that sound file into a source
    let mut source = Decoder::new(file).unwrap();
    //add the sound to the sink
    sink.append(source);
}
