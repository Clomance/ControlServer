use axum::{
    Json, Router, extract::{DefaultBodyLimit, Multipart, Path, State}, http::{Method, Response}, response::{Html, IntoResponse}, routing::{delete, get, post}
};
use serde::{Deserialize, Serialize};
use tower_http::{cors::{Any, CorsLayer}, services::ServeDir};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, RwLock};

use crate::{TRACKS_DIRECTORY, audio::{AudioPlayerCommand, AudioPlayerCommander}, database::gen_id, event_queue::{Event, EventSender}, log, playlists::{PlaylistInfo, Playlists}};



#[derive(Clone)]
struct AppState {
    global_volume: Arc<Mutex<f32>>,

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
        global_volume: Arc::new(Mutex::new(global_volume)),

        current_track,
        audio_player,
        playlists,
        event_queue
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers(Any);

    let serve_dir = ServeDir::new(PathBuf::from(TRACKS_DIRECTORY));

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
        .route("/audio_player/playlist/{playlist_id}", get(get_playlist))
        .route("/audio_player/playlist", post(post_playlist))
        .route("/audio_player/playlist/{playlist_id}", delete(delete_playlist))
        .route("/audio_player/playlist/{playlist_id}/volume/{volume}", post(post_playlist_volume))

        // Треки
        .route("/audio_player/playlist/{playlist_id}/track/{track_id}/volume/{volume}", post(post_track_volume))
        .route("/audio_player/playlist/{playlist_id}/track", post(post_track))
        .route("/audio_player/playlist/{playlist_id}/track/{track_id}", delete(delete_track))

        // static files
        .nest_service("/audio_player/records", serve_dir)
        .with_state(state)
        .layer(cors)
        .layer(DefaultBodyLimit::disable());

    log!("Server listening on http://{address}:{port}");

    let address = format!("{address}:{port}");
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}



/// 
async fn index() -> impl IntoResponse {
    if let Ok(buffer) = std::fs::read("./web/music/index.html") {
        Html(buffer)
    }
    else {
        Html("Error".as_bytes().to_vec())
    }
}



/// 
async fn get_current_track(State(state): State<AppState>) -> impl IntoResponse {
    let current_track = state.current_track.read().await.clone();
    Json(serde_json::json!({ "track": current_track }))
}

/// 
async fn post_current_track_action(Path(action): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    match action.as_str() {
        "play" => {
            state.audio_player.send(AudioPlayerCommand::Unpause)
        }

        "next" => {
            state.event_queue.send(Event::Next)
        }

        "pause" => {
            state.audio_player.send(AudioPlayerCommand::Pause)
        }

        _ => {}
    }

    log!("Action: {action}");

    Response::builder().status(200).body("".to_string()).unwrap()
}

/// 
async fn get_global_volume(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({ "volume": state.global_volume.lock().await.clone() }))
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

    *state.global_volume.lock().await = volume;

    Response::builder().status(200).body("".to_string()).unwrap()
}



/// 
async fn get_playlists(State(state): State<AppState>) -> impl IntoResponse {
    let playlists: Vec<PlaylistInfo> = state.playlists.info_list().await;
    Json(serde_json::json!({ "playlists": playlists }))
}

/// 
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


#[derive(Serialize)]
#[derive(Deserialize)]
struct PlaylistPayload {
    name: String,
    // directory: String,
    // period: u64
}
/// 
async fn post_playlist(State(state): State<AppState>, Json(payload): Json<PlaylistPayload>) -> impl IntoResponse {
    let name = payload.name;
    let period = 0u64;

    let id = gen_id();

    state.event_queue.send(Event::AddPlaylist { id: id.clone(), name, period });

    Json(serde_json::json!({ "playlist": null }))
}

/// 
async fn delete_playlist(Path(playlist): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    state.event_queue.send(Event::DeletePlaylist(playlist));

    Response::builder().status(200).body("".to_string()).unwrap()
}

/// 
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



/// 
async fn post_track_volume(
    Path((playlist, track, volume)): Path<(String, String, String)>,
    State(state): State<AppState>
) -> impl IntoResponse {
    let mut volume: f32 = volume.parse().unwrap();

    if volume > 1f32 {
        volume = 1f32
    }
    else if volume < 0f32 {
        volume = 0f32
    }

    state.event_queue.send(Event::SetTrackVolume{ playlist, track, volume });

    Response::builder().status(200).body("".to_string()).unwrap()
}

/// 
async fn post_track(
    Path(playlist): Path<String>,
    State(state): State<AppState>,
    mut multipart: Multipart
) -> impl IntoResponse {
    let mut name = String::new();
    let mut file = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();

        if field_name == "name" {
            if let Ok(text) = field.text().await {
                name = text;
            }
        }
        else if field_name == "file" {
            if let Some(file_name) = field.file_name() {
                name = file_name.to_string()
            }
            if let Ok(data) = field.bytes().await {
                file = data.to_vec()
            }
        }
    }

    if name.is_empty() || file.is_empty() {
        return Json(serde_json::json!({ "track": null }))
    }

    let id = gen_id();

    state.event_queue.send(Event::AddTrack { id: id.clone(), playlist, name, file });

    Json(serde_json::json!({ "track": id }))
}

/// 
async fn delete_track(
    Path((playlist, track)): Path<(String, String)>,
    State(state): State<AppState>
) -> impl IntoResponse {
    state.event_queue.send(Event::DeleteTrack { playlist, track });
    Response::builder().status(200).body("".to_string()).unwrap()
}