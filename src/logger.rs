use std::{
    fmt::Display, fs::{self, File, OpenOptions}, io::Write, path::Path, sync::{Mutex, OnceLock, mpsc::{Receiver, Sender}}
};

use chrono::Local;



pub const MAX_LOG_SIZE: u64 = 1_000_000;



static LOGGER: OnceLock<Logger> = OnceLock::new();



#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        $crate::logger::get_logger().send_message(
            $crate::logger::MessageLevel::Common,
            format!($($arg)*),
        );
    }};
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        $crate::logger::get_logger().send_message(
            $crate::logger::MessageLevel::Error,
            format!($($arg)*),
        );
    }};
}

#[macro_export]
macro_rules! log_panic {
    ($($arg:tt)*) => {{
        $crate::logger::get_logger().send_message(
            $crate::logger::MessageLevel::Panic,
            format!($($arg)*),
        );
    }};
}



pub const LOG_FILE: &'static str = "logs.txt";
pub const LOG_FILE_OLD: &'static str = "logs.old";



pub enum MessageLevel {
    Common,
    Error,
    Panic
}

impl Display for MessageLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            MessageLevel::Common => "COMMON",
            MessageLevel::Error => "ERROR",
            MessageLevel::Panic => "PANIC"
        };
        f.write_str(text)
    }
}



pub struct Message {
    level: MessageLevel,
    text: String
}



pub struct Logger {
    sender: Mutex<Sender<Message>>
}

impl Logger {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<Message>();

        std::thread::spawn(move || logger_thread(receiver));

        Self {
            sender: Mutex::new(sender)
        }
    }

    pub fn send_message(&self, level: MessageLevel, text: String) {
        if let Ok(sender) = self.sender.lock() {
            let _ = sender.send(Message { level, text });
        }
    }
}


fn open_file(path: &str) -> File {
    OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .expect("cannot open log file")
}


fn logger_thread(receiver: Receiver<Message>) {
    let mut file = open_file(LOG_FILE);

    while let Ok(message) = receiver.recv() {
        let time = Local::now().format("%d.%m.%Y %H:%M:%S").to_string();
        let line = format!("{} [{}] {}\n", time, message.level, message.text);

        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();


        let Ok(metadata) = file.metadata() else { continue };
        let size = metadata.len();

        if size < MAX_LOG_SIZE { continue }

        drop(file);

        if Path::new(&LOG_FILE_OLD).exists() {
            let _ = fs::remove_file(&LOG_FILE_OLD);
        }

        let _ = fs::rename(LOG_FILE, &LOG_FILE_OLD);

        file = open_file(LOG_FILE);

    }
}



pub fn get_logger() -> &'static Logger {
    LOGGER.get_or_init(|| Logger::new())
}
