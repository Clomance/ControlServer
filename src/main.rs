mod audio;
use audio::*;
mod sample_rate_transformer;
mod event_queue;
use event_queue::*;
mod playlists;
use playlists::*;
mod web;
use tokio::sync::RwLock;
use web::*;
mod scheduler;
use scheduler::*;
mod server_config;
use server_config::*;
mod database;
use database::*;
use std::{sync::Arc, thread::sleep, time::Duration};
mod logger;
use logger::*;


pub static PLAYLISTS_DATABASE_FILE_PATH: &str = "database/playlists.json";
pub static TRACKS_DATABASE_FILE_PATH: &str = "database/tracks.json";
pub static TRACKS_DIRECTORY: &str = "records";



#[tokio::main]
async fn main() {
    log!("The scheduled audio player started!");


    println!("Parsing configs");
    let mut config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            println!("{}", e.text);
            return
        }
    };

    let mut database = FileDatabase::new().await;
    let playlists = database.load().await;

    println!("Loading playlists");
    let playlists= Playlists::new(playlists);


    let (event_queue, event_sender) = EventQueue::new();

    println!("Make up schedule");
    let mut scheduler = Scheduler::new(Vec::new(), event_sender.clone());
    for playlist in playlists.list().await {
        scheduler.commander().add_task(SchedulerTask::new(&playlist.id, playlist.period));
    }

    let mut global_volume = config.volume;
    let mut current_playlist = String::new();
    let mut current_track_id = String::new();
    let current_track: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
    let audio_player = AudioPlayer::new(event_sender.clone());

    run_webserver(
        config.web_server_address.clone(),
        config.web_server_port.clone(),
        global_volume,
        playlists.clone(),
        current_track.clone(),
        audio_player.commander(),
        event_sender.clone()
    );

    println!("Run");
    scheduler.commander().next();
    event_queue.run(async |event| {
        println!("{event}");
        match event {
            Event::None => {
                *current_track.write().await = None;
                sleep(Duration::from_secs(1));
                scheduler.commander().next();
            }

            Event::Next => {
                audio_player.commander().send(AudioPlayerCommand::Stop);
                scheduler.commander().next();
            }

            Event::PlayTrack(task) => {
                let playlist = playlists.get(&task).await;

                if let Some(playlist) = playlist {
                    let playlist_lock = playlist.read().await;
                    let track = playlist_lock.get_random();
                    let playlist_volume = playlist_lock.volume;

                    if let Some(track) = track {
                        println!("PlayTrack name: {}", track.name);

                        current_playlist = playlist_lock.id.clone();
                        current_track_id = track.id.clone();
                        drop(playlist_lock);

                        let name = track.name;
                        let path = track.path;

                        *current_track.write().await = Some(name.to_string());

                        let volume = global_volume * playlist_volume * track.volume;
                        audio_player.commander().send(AudioPlayerCommand::Play{ path, volume } );
                    }
                    else {
                        println!("PlayTrack no track");

                        sleep(Duration::from_secs(1));
                        scheduler.commander().next();
                    }
                }
                else {
                    sleep(Duration::from_secs(1));
                    scheduler.commander().next();
                }
            }

            Event::SetGlobalVolume(volume) => {
                global_volume = volume;
                config.volume = volume;

                if let Some(playlist) = playlists.get(&current_playlist).await {
                    let playlist_lock = playlist.read().await;

                    if let Some(track_volume) = playlist_lock.get_track_volume(current_track_id.clone()) {
                        let volume = playlist_lock.volume * track_volume * global_volume;
                        audio_player.commander().send(AudioPlayerCommand::Volume(volume));
                    }
                }

                config.save().await;
            }

            Event::SetPlaylistVolume{ playlist, volume} => {
                if let Some(playlist) = playlists.get(&playlist).await {
                    let mut playlist_lock = playlist.write().await;
                    playlist_lock.volume = volume;

                    let volume = playlist_lock.volume * volume;
                    audio_player.commander().send(AudioPlayerCommand::Volume(volume));

                    let playlists = playlists.list().await;
                    let configs: Vec<PlaylistConfig> = playlists.iter().map(|p| p.to_config()).collect();
                    let _ = database.playlists.save(&configs).await;
                }
            }

            Event::SetTrackVolume{ playlist, track, volume} => {
                if let Some(playlist) = playlists.get(&playlist).await {
                    let mut playlist_lock = playlist.write().await;

                    playlist_lock.set_track_volume(track.clone(), volume);

                    if current_track_id == track {
                        let volume = playlist_lock.volume * global_volume * volume;
                        audio_player.commander().send(AudioPlayerCommand::Volume(volume));
                    }
                }
            }

            Event::AddPlaylist { id, name, period } => {
                playlists.add(id.clone(), name, period).await;

                scheduler.commander().add_task(SchedulerTask::new(&id, period));

                let playlists = playlists.list().await;
                let configs: Vec<PlaylistConfig> = playlists.iter().map(|p| p.to_config()).collect();
                let _ = database.playlists.save(&configs).await;
            }

            Event::DeletePlaylist(id) => {
                playlists.delete(id.clone()).await;

                scheduler.commander().delete_task(id);

                let playlists = playlists.list().await;
                let configs: Vec<PlaylistConfig> = playlists.iter().map(|p| p.to_config()).collect();
                let _ = database.playlists.save(&configs).await;
            }

            Event::AddTrack { id, playlist, name, file } => {
                playlists.add_track(id, playlist, name, file).await
            }

            Event::DeleteTrack { playlist, track } => {
                playlists.delete_track(track, playlist).await;
            }
        }
    }).await;

    scheduler.join();
}