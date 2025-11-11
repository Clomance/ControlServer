use axum::{
    Json, Router, extract::{DefaultBodyLimit, Multipart, Path, State}, http::{Method, Response}, response::{Html, IntoResponse}, routing::{delete, get, post}
};
use serde::{Serialize};
use tower_http::{cors::{Any, CorsLayer}, services::ServeDir};
use std::{path::{Path as StdPath, PathBuf}, sync::Arc};
use tokio::sync::RwLock;

use crate::{audio::{AudioPlayerCommand, AudioPlayerCommander}, database::gen_id, event_queue::{Event, EventSender}, playlists::{PlaylistInfo, Playlists}};



#[derive(Clone)]
struct AppState {
    global_volume: f32,

    current_track: Arc<RwLock<Option<String>>>,
    audio_player: AudioPlayerCommander,
    playlists: Playlists,
    event_queue: EventSender
}



pub fn run_webserver(
    address: String,
    port: u16,
    global_volume: f32,
    playlists: Playlists,
    current_track: Arc<RwLock<Option<String>>>,
    audio_player: AudioPlayerCommander,
    event_queue: EventSender
) {
    std::thread::spawn(move || {
        let thread = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        thread.block_on(
            run_webserver_(
                address,
                port,
                global_volume,
                playlists,
                current_track,
                audio_player,
                event_queue
            )
        );
    });
}


pub async fn run_webserver_(
    address: String,
    port: u16,
    global_volume: f32,
    playlists: Playlists,
    current_track: Arc<RwLock<Option<String>>>,
    audio_player: AudioPlayerCommander,
    event_queue: EventSender
) {
    let state = AppState {
        global_volume,

        current_track,
        audio_player,
        playlists,
        event_queue
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers(Any);

    let serve_dir = ServeDir::new(PathBuf::from("./records"));

    let app = Router::new()

        // Страницы
        .route("/", get(index))
        // .route("/audio_player/", get())

        // Общие
        .route("/audio_player/current", get(get_current_track))
        .route("/audio_player/current/action/{action}", post(post_current_track_action))
        .route("/audio_player/volume", get(get_global_volume))
        .route("/audio_player/volume/{volume}", post(post_global_volume))

        // Плейлисты
        .route("/audio_player/playlists", get(get_playlists))
        .route("/audio_player/playlist/{playlist}", get(get_playlist))
        .route("/audio_player/playlist/{playlist}", post(post_playlist))
        .route("/audio_player/playlist/{playlist}", delete(delete_playlist))
        .route("/audio_player/playlist/{playlist}/volume/{volume}", post(post_playlist_volume))

        // Треки
        .route("/audio_player/playlist/{playlist}/track/{track}/volume/{volume}", post(post_track_volume))
        .route("/audio_player/playlist/{playlist}/track/{track}", post(post_track))
        .route("/audio_player/playlist/{playlist}/track/{track}", delete(delete_track))

        // static files
        .nest_service("/records", serve_dir)
        .with_state(state)
        .layer(cors)
        .layer(DefaultBodyLimit::disable());

    println!("Server listening on http://{address}:{port}");

    let address = format!("{address}:{port}");
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}


async fn index() -> impl IntoResponse {
    if let Ok(buffer) = std::fs::read("./web/music/index.html") {
        Html(buffer)
    }
    else {
        Html("Error".as_bytes().to_vec())
    }
}



async fn get_current_track(State(state): State<AppState>) -> impl IntoResponse {
    let current_track = state.current_track.read().await.clone();
    Json(serde_json::json!({ "track": current_track }))
}

async fn post_current_track_action(Path(action): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    match action.as_str() {
        "play" => {
            state.audio_player.send(AudioPlayerCommand::Unpause)
        }

        "playnext" => {
            state.event_queue.send(Event::Next)
        }

        "pause" => {
            state.audio_player.send(AudioPlayerCommand::Pause)
        }

        _ => {}
    }

    println!("Action: {action}");

    Response::builder().status(200).body("".to_string()).unwrap()
}

async fn get_global_volume(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({ "volume": state.global_volume }))
}

async fn post_global_volume(Path(volume): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    let mut volume: f32 = volume.parse().unwrap();

    if volume > 1f32 {
        volume = 1f32
    }
    else if volume < 0f32 {
        volume = 0f32
    }

    state.event_queue.send(Event::SetGlobalVolume(volume));

    Response::builder().status(200).body("".to_string()).unwrap()
}



async fn get_playlists(State(state): State<AppState>) -> impl IntoResponse {
    let playlists: Vec<PlaylistInfo> = state.playlists.info_list().await;
    Json(serde_json::json!({ "playlists": playlists }))
}

async fn get_playlist(Path(playlist): Path<String>, State(state): State<AppState>) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let Some(playlist) = state.playlists.get(&playlist).await else {
        return Err((axum::http::StatusCode::NOT_FOUND, format!("playlist not found")));
    };

    let playlist = playlist.read().await.clone();

    Ok(Json(
        serde_json::json!(
            {
                "playlist": playlist
            }
        )
    ))
}

async fn post_playlist(Path(playlist): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    // Placeholder for future functionality
    println!("POST playlist: {}", playlist);
    // TODO: Implement playlist modification logic here

    let id = gen_id();
    Json(serde_json::json!({ "playlist": id }))
}

async fn delete_playlist(Path(playlist): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    // Placeholder for future functionality
    println!("DELETE playlist: {}", playlist);
    // TODO: Implement playlist deletion logic here
    Response::builder().status(200).body("".to_string()).unwrap()
}

async fn post_playlist_volume(Path((playlist, volume)): Path<(String, String)>, State(state): State<AppState>) -> impl IntoResponse {
    let mut volume: f32 = volume.parse().unwrap();

    if volume > 1f32 {
        volume = 1f32
    }
    else if volume < 0f32 {
        volume = 0f32
    }

    state.event_queue.send(Event::SetPlaylistVolume{ playlist, volume });

    Response::builder().status(200).body("".to_string()).unwrap()
}



async fn post_track_volume(Path((playlist, track, volume)): Path<(String, String, String)>, State(state): State<AppState>) -> impl IntoResponse {
    let mut volume: f32 = volume.parse().unwrap();

    if volume > 1f32 {
        volume = 1f32
    }
    else if volume < 0f32 {
        volume = 0f32
    }

    // Placeholder for future functionality
    state.event_queue.send(Event::SetTrackVolume{ playlist, track, volume });

    Response::builder().status(200).body("".to_string()).unwrap()
}

async fn post_track(Path((playlist, track)): Path<(String, String)>, State(state): State<AppState>, multipart: Multipart) -> impl IntoResponse {
    // Placeholder for future functionality

    // TODO: Implement track modification logic here
    let id = gen_id();
    Json(serde_json::json!({ "track": id }))
}

async fn delete_track(Path((playlist, track)): Path<(String, String)>, State(state): State<AppState>) -> impl IntoResponse {
    // Placeholder for future functionality
    Response::builder().status(200).body("".to_string()).unwrap()
}