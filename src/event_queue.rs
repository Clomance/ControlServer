use std::{fmt::Display, sync::mpsc::{Receiver, SyncSender}};



pub enum Event {
    None,
    Next,
    PlayTrack(String),
    SetGlobalVolume(f32),
    SetPlaylistVolume {
        playlist: String,
        volume: f32
    },
    SetTrackVolume {
        playlist: String,
        track: String,
        volume: f32
    },
    AddPlaylist {
        id: String,
        name: String,
        period: u64
    },
    DeletePlaylist(String),
    AddTrack {
        id: String,
        playlist: String,
        name: String,
        file: Vec<u8>
    },
    DeleteTrack {
        playlist: String,
        track: String
    }
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::None =>
                f.write_fmt(format_args!("None")),

            Event::Next =>
                f.write_fmt(format_args!("Next")),

            Event::PlayTrack(track) =>
                f.write_fmt(format_args!("PlayTrack({})", track)),

            Event::SetGlobalVolume(volume) =>
                f.write_fmt(format_args!("SetGlobalVolume({})", volume)),

            Event::SetPlaylistVolume { playlist, volume } =>
                f.write_fmt(format_args!("SetPlaylistVolume{{ playlist: {}, volume: {} }}", playlist, volume)),

            Event::SetTrackVolume { playlist, track, volume } =>
                f.write_fmt(format_args!("SetPlaylistVolume{{ playlist: {}, track: {}, volume: {} }}", playlist, track, volume)),

            Event::AddPlaylist { id, name, period } =>
                f.write_fmt(format_args!("AddPlaylist{{ id: {}, name: {}, period: {} }}", id, name, period)),

            Event::DeletePlaylist(playlist) =>
                f.write_fmt(format_args!("DeletePlaylist({})", playlist)),

            Event::AddTrack { id, playlist, name, file } =>
                f.write_fmt(format_args!("AddTrack{{ id: {}, playlist: {}, name: {}, file_size: {} }}", id, playlist, name, file.len())),

            Event::DeleteTrack { playlist, track } =>
                f.write_fmt(format_args!("DeleteTrack{{ playlist: {}, track: {} }}", playlist, track)),
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