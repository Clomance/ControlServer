use std::sync::mpsc::{Receiver, SyncSender};



#[derive(Debug)]
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