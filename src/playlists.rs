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
    pub volume: f32
}



#[derive(Serialize, Deserialize, Clone)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub directory: PathBuf,
    pub volume: f32,
    pub period: u64,
    pub tracks: Vec<Track>
}

impl Playlist {
    pub fn new(config: &PlaylistConfig) -> Playlist {
        let tracks = Vec::new();

        Self {
            id: config.id.clone(),
            name: config.name.clone(),
            directory: PathBuf::from(&config.directory),
            volume: config.volume,
            period: config.period,
            tracks
        }
    }

    pub fn short_info(&self) -> PlaylistInfo {
        PlaylistInfo::new(self)
    }

    pub fn get_random(&self) -> Option<Track> {
        if self.tracks.is_empty() { return None }
        let len = self.tracks.len();

        let mut rng = rand::rng();
        let index = rng.random_range(0..len);

        self.tracks.get(index).take().map(|f| f.clone())
    }

    pub fn names(&self) -> Vec<String> {
        let mut names = Vec::new();

        for track in &self.tracks {
            names.push(track.name.to_string());
        }

        names
    }

    pub fn remove(&mut self, id: String) {
        for c in 0..self.tracks.len() {
            if self.tracks[c].id == id {
                self.tracks.remove(c);
                return
            }
        }
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
        path.push(id.clone());
        let _ = DirBuilder::new().recursive(true).create(path).await;

        let mut lock = self.inner.write().await;
        let playlist = Playlist {
            id: id.clone(),
            name,
            directory: PathBuf::from(id),
            volume: 0.5,
            period,
            tracks: Vec::new()
        };
        lock.insert(playlist.id.clone(), Arc::new(RwLock::new(playlist)));
    }

    // pub async fn delete(&self, id: &str) {
    //     let mut lock = self.inner.write().await;
    //     lock.remove(id);
    // }

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