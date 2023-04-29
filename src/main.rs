//all the imports and configs
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::{
    AppHandle, CustomMenuItem, SystemTray, SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem,
};

extern crate dirs;
extern crate single_instance;
use crossbeam_channel::{tick, unbounded, Receiver, Sender};
use native_dialog::{FileDialog, MessageDialog, MessageType};
use rodio::source::Source;
use rodio::{Decoder, OutputStream, Sink};
use single_instance::SingleInstance;
use std::env;
use std::io::Read;
use std::io::Write;
use std::io::{self, BufRead};
use std::iter::Iterator;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::{fs, thread};
use std::{fs::File, io::BufReader};

static mut MUSIC_THREAD: Option<Sender<String>> = None; //the sender side of the channel to communicate with the music thread
static mut PLAYING: bool = true; //whether or not the player is playing

//main method
fn main() {
    //read the tracks directory
    let trackDir = readTrackDir();
    println!("{}", trackDir);

    //make sure the program is single instance
    let instance = SingleInstance::new("whatever").unwrap();
    assert!(instance.is_single());
    //define the tray menu
    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("quit".to_string(), "Quit"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new(
            "changeTrackDir".to_string(),
            trackDir.to_owned(),
        ))
        .add_item(CustomMenuItem::new("playPause-toggle".to_string(), "Pause"));

    //create a tray with the menu defined above
    let tray = SystemTray::new().with_menu(tray_menu);

    //create a thread to run the music
    unsafe { MUSIC_THREAD = Option::from(createMusicThread(trackDir.to_owned())) };

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
fn createMusicThread(trackDir: String) -> Sender<String> {
    println!("creating music thread");
    let (tx, rx): (Sender<String>, Receiver<String>) = unbounded();
    let handle = thread::spawn(move || {
        let mut trackDirectory = trackDir.to_owned();
        let lofiDirectory = unsafe { trackDirectory.to_owned() } + "\\lofiMusic"; //directory for lofi music
        let backgroundDirectory = unsafe { trackDirectory.to_owned() } + "\\backgroundSound"; //direcory for background music

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
                        }
                        "PAUSE" => {
                            println!("pausing");
                            backgroundSink.pause();
                            lofiSink.pause();
                        }
                        _ => {
                            if(msg.starts_with("trackDir:")){
                                let tmp = (msg.strip_prefix("trackDir:").unwrap());
                                println!("new dir: {}", tmp.to_owned());
                                trackDirectory = tmp.to_owned();
                            }
                        }
                    }
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
                "changeTrackDir" => {
                    println!("selecting track directory");
                    let inputPath = FileDialog::new()
                        .set_filename("select track directory")
                        .set_location("~/Documents")
                        .show_open_single_dir()
                        .unwrap();

                    let trackPath = match inputPath {
                        Some(inputPath) => inputPath,
                        None => return,
                    };
                    let trackDir = trackPath.to_str().unwrap();
                    unsafe {
                        (&MUSIC_THREAD.as_ref()).unwrap().send(
                            ("trackDir:".to_string() + trackDir.to_owned().as_str()).to_string(),
                        )
                    };
                    updateTrackDir(trackDir.to_owned());
                    item_handle.set_title(trackDir).unwrap();
                }
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

/**
 * function to read the track directory path
 */
fn readTrackDir() -> String {
    println!("getting track directory");
    let appData = dirs::config_dir().unwrap().display().to_string();
    let appDataStr = appData.to_owned();
    let mut binding = Path::new(appDataStr.as_str()).join("\\LofiPlayer");
    let appDir = binding.as_path();
    let binding = Path::new(appDir).join("tracks");
    let mut trackDir = binding.as_path();
    let fileName = "trackDirPath.txt".to_string();
    let filePath = Path::new(appDir).join(fileName.as_str());

    let trackDirBuf = if (appDir.exists()) {
        //if the app directory exists
        let mut file = fs::File::open(filePath.display().to_string()).unwrap();
        let reader = BufReader::new(&mut file);
        let line = reader.lines().next().unwrap().unwrap();
        let lineCopy = line.clone();
        PathBuf::from(lineCopy.as_str())
    } else {
        //if the app directory doesn't exist
        println!("appDir doesnt exist, creating it");
        //create the app directory
        fs::create_dir(appDir).unwrap();
        //create the tracks directory
        fs::create_dir(trackDir).unwrap();
        //create the lofi and background directories
        fs::create_dir(trackDir.join("lofiMusic")).unwrap();
        fs::create_dir(trackDir.join("backgroundSound")).unwrap();
        //create the trackDirPath.txt file
        let mut newFile = fs::File::create(filePath).unwrap();
        newFile
            .write_all(trackDir.display().to_string().as_bytes())
            .unwrap();
        PathBuf::from(trackDir)
    };

    trackDir = trackDirBuf.as_path();
    return trackDir.display().to_string();
}

/**
 * a function to update the tack directory
 */
fn updateTrackDir(path: String) {
    println!("updating track directory");
    let fileName = "trackDirPath.txt".to_string();
    let filePath = fs::canonicalize(&PathBuf::from(fileName.clone())).unwrap();
    fs::write(filePath.as_path(), path).unwrap();
}
