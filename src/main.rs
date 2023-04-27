//all the imports and configs
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::{
    AppHandle, CustomMenuItem, SystemTray, SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem,
};
extern crate single_instance;
use crossbeam_channel::{tick, unbounded, Receiver, Sender};
use rodio::source::Source;
use rodio::{Decoder, OutputStream, Sink};
use single_instance::SingleInstance;
use std::iter::Iterator;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::{fs, thread};
use std::{fs::File, io::BufReader};

static mut MUSIC_THREAD: Option<Sender<String>> = None; //the sender side of the channel to communicate with the music thread
static mut PLAYING: bool = true; //whether or not the player is playing

//main method
fn main() {
    //make sure the program is single instance
    let instance = SingleInstance::new("whatever").unwrap();
    assert!(instance.is_single());
    //define the tray menu
    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("quit".to_string(), "Quit"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("playPause-toggle".to_string(), "Pause"));

    //create a tray with the menu defined above
    let tray = SystemTray::new().with_menu(tray_menu);

    //create a thread to run the music
    unsafe { MUSIC_THREAD = Option::from(createMusicThread()) };

    //start the tray application
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

/**
 * function to create a thread that will handle all the music
 * returns Sender<String> a way to pass messages to the thread
 */
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
                let opt = getRndTrack(&backgroundDirectory);
                match opt {
                    None => {}
                    Some(_) => {
                        backgroundFile = opt.unwrap();
                        playTrack(&backgroundSink, backgroundFile);
                    }
                }
            }
            //play the lofi music
            if (lofiSink.empty()) {
                let opt = getRndTrack(&lofiDirectory);
                match opt {
                    None => {}
                    Some(_) => {
                        lofiFile = opt.unwrap();
                        playTrack(&lofiSink, lofiFile);
                    }
                }
            }
        }

        println!("somehow broke out of the loop");
    });
    return tx;
}

/**
 * a function to define the tray menu button actions
 * this function gets passed to the tauri builder
 */
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

/**
 * function to get a random file from a given directory
 * directory - the directory to get a random file from
 */
fn getRndTrack(directory: &str) -> Option<File> {
    let mut rng = rand::thread_rng();
    let files = fs::read_dir(directory).unwrap();
    let file;

    let itr = rand::seq::IteratorRandom::choose(files, &mut rand::thread_rng());
    match itr {
        None => return None,
        Some(_) => {
            let binding = itr.unwrap().unwrap().path();
            println!("got track: {}", binding.to_str().unwrap());
            file = binding.to_str().unwrap();

            return Option::from(File::open(file).unwrap());
        }
    }
    // let binding = itr.unwrap().unwrap().path();
    // println!("got track: {}", binding.to_str().unwrap());
    // file = binding.to_str().unwrap();

    // return Option::from(File::open(file).unwrap());
}

/**
 * function to play a given track on a given sink
 * sink - the sink to play the track
 * file - the file to play
 */
fn playTrack(sink: &Sink, file: File) {
    println!("playing track");
    // Decode that sound file into a source
    let mut source = Decoder::new(file).unwrap();
    //add the sound to the sink
    sink.append(source);
}
