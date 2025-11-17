use tokio::fs::{DirBuilder, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json;
use tokio::fs::File;

use crate::{PLAYLISTS_DATABASE_FILE_PATH, TRACKS_DATABASE_FILE_PATH, TRACKS_DIRECTORY};
use crate::playlists::{Playlist, PlaylistConfig, Track, TrackConfig};



pub fn gen_id() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let rnd: u32 = rand::random();
    format!("{:x}-{:x}", nanos as u128, rnd)
}


pub struct PlaylistsDatabase {
    path: String,
    file: File
}

impl PlaylistsDatabase {
    pub async fn open<P: AsRef<Path>>(path: P) -> Result<PlaylistsDatabase, String> {
        let file = OpenOptions::new().read(true).write(true).create(true).open(&path).await.map_err(|e| format!("failed open db: {}", e))?;

        let path = path.as_ref().as_os_str().to_string_lossy().to_string();

        Ok(
            PlaylistsDatabase {
                path,
                file
            }
        )
    }

    pub async fn load(&mut self) -> Result<Vec<PlaylistConfig>, String> {
        let mut data = String::new();
        self.file.read_to_string(&mut data).await.map_err(|e| format!("failed read db: {}", e))?;

        let data = serde_json::from_str::<Vec<PlaylistConfig>>(&data).map_err(|e| format!("failed parse db json: {}", e))?;

        Ok(data)
    }

    pub async fn save(&self, playlists: &[PlaylistConfig]) -> Result<(), String> {
        let tmp = format!("{}.tmp", &self.path);
        let json = serde_json::to_string_pretty(playlists).map_err(|e| format!("failed serialize: {}", e))?;
        let mut f = File::create(&tmp).await.map_err(|e| format!("failed create tmp file: {}", e))?;
        f.write_all(json.as_bytes()).await.map_err(|e| format!("failed write tmp file: {}", e))?;
        tokio::fs::rename(&tmp, &self.path).await.map_err(|e| format!("failed rename tmp file: {}", e))?;
        Ok(())
    }
}



pub struct TracksDatabase {
    path: String,
    file: File
}

impl TracksDatabase {
    pub async fn open<P: AsRef<Path>>(path: P) -> Result<TracksDatabase, String> {
        let file = OpenOptions::new().read(true).write(true).create(true).open(&path).await.map_err(|e| format!("failed open db: {}", e))?;

        let path = path.as_ref().as_os_str().to_string_lossy().to_string();

        Ok(
            TracksDatabase {
                path,
                file
            }
        )
    }

    pub async fn load(&mut self) -> Result<Vec<TrackConfig>, String> {
        let mut data = String::new();
        self.file.read_to_string(&mut data).await.map_err(|e| format!("failed read db: {}", e))?;

        let data = serde_json::from_str::<Vec<TrackConfig>>(&data).map_err(|e| format!("failed parse db json: {}", e))?;

        Ok(data)
    }

    pub async fn save(&self, track: &[TrackConfig]) -> Result<(), String> {
        let tmp = format!("{}.tmp", &self.path);
        let json = serde_json::to_string_pretty(track).map_err(|e| format!("failed serialize: {}", e))?;
        let mut f = File::create(&tmp).await.map_err(|e| format!("failed create tmp file: {}", e))?;
        f.write_all(json.as_bytes()).await.map_err(|e| format!("failed write tmp file: {}", e))?;
        tokio::fs::rename(&tmp, &self.path).await.map_err(|e| format!("failed rename tmp file: {}", e))?;
        Ok(())
    }
}



pub struct FileDatabase {
    pub playlists: PlaylistsDatabase,
    pub tracks: TracksDatabase
}

impl FileDatabase {
    pub async fn new() -> FileDatabase {
        let playlists = PlaylistsDatabase::open(PLAYLISTS_DATABASE_FILE_PATH).await.unwrap();
        let tracks = TracksDatabase::open(TRACKS_DATABASE_FILE_PATH).await.unwrap();

        Self {
            playlists,
            tracks
        }
    }

    pub async fn load(&mut self) -> Vec<Playlist> {
        let playlists_configs = self.playlists.load().await.unwrap();
        let mut tracks_configs = self.tracks.load().await.unwrap();

        let mut playlists = Vec::new();

        for playlist_config in playlists_configs {
            let mut playlist_directory_path = PathBuf::from(TRACKS_DIRECTORY);
            playlist_directory_path.push(&playlist_config.directory);

            let _ = DirBuilder::new().recursive(true).create(&playlist_directory_path).await.unwrap();

            let mut tracks = Vec::new();
            if let Ok(mut directory) = tokio::fs::read_dir(&playlist_directory_path).await {
                while let Ok(Some(entry)) = directory.next_entry().await {
                    let Ok(metadata) = entry.metadata().await else { continue };

                    if !metadata.is_file() { continue }

                    let Ok(path) = std::path::absolute(entry.path()) else { continue };

                    let Some(file_name) = path.file_name() else { continue };
                    let track_name = file_name.to_string_lossy().to_string();

                    let mut track_path = playlist_directory_path.clone();
                    track_path.push(&track_name);

                    let mut track = None;
                    for c in 0..tracks_configs.len() {
                        let track_config = &tracks_configs[c];

                        if track_config.playlist == playlist_config.id && track_config.name == track_name {
                            track = Some(tracks_configs.remove(c))
                        }
                    }

                    if let Some(track) = track {
                        let track = Track {
                            id: track.id,
                            name: track.name,
                            playlist: playlist_config.id.clone(),
                            volume: track.volume,
                            path: track_path
                        };

                        tracks.push(track);
                    }
                    else {
                        let track = Track {
                            id: gen_id(),
                            name: track_name,
                            playlist: playlist_config.id.clone(),
                            volume: 0.5,
                            path: track_path
                        };

                        tracks.push(track);

                       
                    }
                }
            }

            let mut playlist = Playlist::new(&playlist_config);
            playlist.tracks = tracks;
            playlists.push(playlist);
        }

        playlists
    }
}