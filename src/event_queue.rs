use std::{fmt::Display, sync::mpsc::{Receiver, SyncSender}};



pub enum Event {
    None,
    Next,
    PlayPlaylist {
        id: String
    },
    PlayTrack {
        playlist_id: String,
        track_id: String
    },
    SetGlobalVolume(f32),
    SetPlaylistVolume {
        id: String,
        volume: f32
    },
    SetTrackVolume {
        playlist_id: String,
        track_id: String,
        volume: f32
    },
    AddPlaylist {
        id: String,
        name: String,
        period: u64
    },
    DeletePlaylist {
        id: String
    },
    AddTrack {
        track_id: String,
        playlist_id: String,
        name: String,
        file: Vec<u8>
    },
    DeleteTrack {
        playlist_id: String,
        track_id: String
    }
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::None =>
                f.write_fmt(format_args!("None")),

            Event::Next =>
                f.write_fmt(format_args!("Next")),

            Event::PlayPlaylist { id} =>
                f.write_fmt(format_args!("PlayPlaylist{{ id: {} }}", id)),

            Event::PlayTrack { playlist_id, track_id } =>
                f.write_fmt(format_args!("PlayTrack{{ playlist_id: {}, track_id: {} }}", playlist_id, track_id)),

            Event::SetGlobalVolume(volume) =>
                f.write_fmt(format_args!("SetGlobalVolume({})", volume)),

            Event::SetPlaylistVolume { id, volume } =>
                f.write_fmt(format_args!("SetPlaylistVolume{{ id: {}, volume: {} }}", id, volume)),

            Event::SetTrackVolume { playlist_id, track_id, volume } =>
                f.write_fmt(format_args!("SetTrackVolume{{ playlist_id: {}, track_id: {}, volume: {} }}", playlist_id, track_id, volume)),

            Event::AddPlaylist { id, name, period } =>
                f.write_fmt(format_args!("AddPlaylist{{ id: {}, name: {}, period: {} }}", id, name, period)),

            Event::DeletePlaylist{ id} =>
                f.write_fmt(format_args!("DeletePlaylist{{ id: {} }}", id)),

            Event::AddTrack { track_id, playlist_id, name, file } =>
                f.write_fmt(format_args!("AddTrack{{ track_id: {}, playlist_id: {}, name: {}, file_size: {} }}", track_id, playlist_id, name, file.len())),

            Event::DeleteTrack { playlist_id, track_id } =>
                f.write_fmt(format_args!("DeleteTrack{{ playlist_id: {}, track_id: {} }}", playlist_id, track_id)),
        }
    }
}



#[derive(Clone)]
pub struct EventSender {
    sender: SyncSender<Event>
}

impl EventSender {
    pub fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}



pub struct EventQueue {
    receiver: Receiver<Event>
}

impl EventQueue {
    pub fn new() -> (EventQueue, EventSender) {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<Event>(64);

        (
            EventQueue {
                receiver
            },

            EventSender {
                sender
            }
        )
    }

    pub async fn run<F: AsyncFnMut(Event)>(&self, mut f: F) {
        while let Ok(event) = self.receiver.recv() {
            f(event).await;
        }
    }
}