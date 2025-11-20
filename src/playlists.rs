use std::{collections::{HashMap}, path::PathBuf, sync::Arc};

use rand::Rng;
use tokio::{fs::DirBuilder, sync::RwLock};
use serde::{Serialize, Deserialize};

use crate::TRACKS_DIRECTORY;



#[derive(Serialize, Deserialize, Clone)]
pub struct TrackConfig {
    pub id: String,
    pub name: String,
    pub playlist: String,
    pub volume: f32
}


#[derive(Serialize, Deserialize, Clone)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub playlist: String,
    pub volume: f32,
    pub path: PathBuf
}

impl Track {
    pub fn to_config(&self) -> TrackConfig {
        TrackConfig {
            id: self.id.clone(),
            name: self.name.clone(),
            playlist: self.playlist.clone(),
            volume: self.volume
        }
    }
}



#[derive(Serialize, Deserialize, Clone)]
pub struct PlaylistInfo {
    pub id: String,
    pub name: String,
    pub tracks_count: usize
}

impl PlaylistInfo {
    pub fn new(playlist: &Playlist) -> PlaylistInfo {
        Self {
            id: playlist.id.clone(),
            name: playlist.name.clone(),
            tracks_count: playlist.tracks.len()
        }
    }
}



#[derive(Serialize, Deserialize, Clone)]
pub struct PlaylistConfig {
    pub id: String,
    pub name: String,
    pub directory: String,
    pub period: u64,
    pub volume: f32,
    pub enabled: bool
}



#[derive(Serialize, Deserialize, Clone)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub directory: PathBuf,
    pub volume: f32,
    pub period: u64,
    pub enabled: bool,
    pub tracks: Vec<Track>
}

impl Playlist {
    pub fn new(config: &PlaylistConfig) -> Playlist {
        Self {
            id: config.id.clone(),
            name: config.name.clone(),
            directory: PathBuf::from(&config.directory),
            volume: config.volume,
            period: config.period,
            enabled: config.enabled,
            tracks: Vec::new()
        }
    }

    pub fn short_info(&self) -> PlaylistInfo {
        PlaylistInfo::new(self)
    }

    pub fn get_track(&self, id: &String) -> Option<Track> {
        let mut volume = None;
        self.tracks.iter().any(|t| {
            let find = &t.id == id;

            if find {
                volume = Some(t.clone())
            }

            find
        });
        volume
    }

    pub fn get_random(&self) -> Option<Track> {
        if self.tracks.is_empty() { return None }
        let len = self.tracks.len();

        let mut rng = rand::rng();
        let index = rng.random_range(0..len);

        self.tracks.get(index).take().map(|f| f.clone())
    }

    pub fn set_track_volume(&mut self, track: String, volume: f32) {
        self.tracks.iter_mut().any(|t| {
            let find = t.id == track;

            if find {
                t.volume = volume
            }

            find
        });
    }

    pub fn get_track_volume(&self, track: String) -> Option<f32> {
        let mut volume = None;
        self.tracks.iter().any(|t| {
            let find = t.id == track;

            if find {
                volume = Some(t.volume)
            }

            find
        });
        volume
    }

    pub fn to_config(&self) -> PlaylistConfig {
        PlaylistConfig {
            id: self.id.clone(),
            name: self.name.clone(),
            directory: self.directory.to_string_lossy().to_string(),
            period: self.period,
            volume: self.volume,
            enabled: self.enabled
        }
    }

    pub fn get_track_configs(&self) -> Vec<TrackConfig> {
        self.tracks.iter().map(|t| t.to_config()).collect()
    }
}



#[derive(Clone)]
pub struct Playlists {
    inner: Arc<RwLock<HashMap<String, Arc<RwLock<Playlist>>>>>
}

impl Playlists {
    pub fn new(configs: Vec<Playlist>) -> Playlists {
        let mut playlists = HashMap::new();

        for config in configs {
            playlists.insert(config.id.clone(), Arc::new(RwLock::new(config)));
        }

        Self {
            inner: Arc::new(RwLock::new(playlists))
        }
    }

    pub async fn add(&self, id: String, name: String, period: u64) {
        let mut path = PathBuf::from(TRACKS_DIRECTORY);
        path.push(name.clone());
        let _ = DirBuilder::new().recursive(true).create(path).await;

        let mut lock = self.inner.write().await;
        let playlist = Playlist {
            id: id.clone(),
            name,
            directory: PathBuf::from(id),
            volume: 0.5,
            period,
            enabled: true,
            tracks: Vec::new()
        };
        lock.insert(playlist.id.clone(), Arc::new(RwLock::new(playlist)));
    }

    pub async fn delete(&self, id: String) {
        let mut lock = self.inner.write().await;
        if let Some(playlist) = lock.remove(&id) {
            let mut path = PathBuf::from(TRACKS_DIRECTORY);
            path.push(&playlist.read().await.directory);
            let _ = tokio::fs::remove_dir_all(path).await;
        }
    }

    pub async fn add_track(&self, track_id: String, playlist_id: String, name: String, file: Vec<u8>) {
        if let Some(playlist) = self.get(&playlist_id).await {
            let mut playlist = playlist.write().await;

            let mut path = PathBuf::from(TRACKS_DIRECTORY);
            path.push(&playlist.directory);
            path.push(&name);

            let _ = tokio::fs::write(&path, file).await;

            let track = Track {
                id: track_id,
                name,
                playlist: playlist_id,
                volume: 0.5,
                path
            };

            playlist.tracks.push(track)
        }
    }

    pub async fn delete_track(&self, track_id: String, playlist_id: String) {
        if let Some(playlist) = self.get(&playlist_id).await {
            let mut playlist = playlist.write().await;

            for c in 0..playlist.tracks.len() {
                let track = &playlist.tracks[c];

                if track.id == track_id {
                    let track = playlist.tracks.remove(c);
                    let _ = tokio::fs::remove_file(track.path).await;
                }
            }
        }
    }

    pub async fn get(&self, id: &str) -> Option<Arc<RwLock<Playlist>>> {
        let lock = self.inner.read().await;
        let playlist = lock.get(id);
        playlist.map(|playlist| playlist.clone())
    }

    pub async fn list(&self) -> Vec<Playlist> {
        let playlists = self.inner.read().await;

        let mut list = Vec::new();
        for playlist in playlists.values() {
            list.push(playlist.read().await.clone())
        }

        list
    }

    pub async fn info_list(&self) -> Vec<PlaylistInfo> {
        let playlists = self.inner.read().await;

        let mut list = Vec::new();
        for playlist in playlists.values() {
            list.push(playlist.read().await.short_info())
        }

        list
    }
}