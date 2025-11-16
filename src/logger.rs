use std::{fmt::Display, fs::OpenOptions, io::Write, sync::mpsc::Sender, thread::JoinHandle};

use chrono::Local;



#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        unsafe {
            use crate::logger::*;
            logger.send_meesage(
                MessageLevel::Common,
                format!($($arg)*)
            )
        }
    };
}

macro_rules! log_error {
    ($($arg:tt)*) => {
        unsafe {
            logger.send_meesage(
                MessageLevel::Error,
                format_args!($($arg:tt)*)
            )
        }
    };
}

macro_rules! log_panic {
    ($($arg:tt)*) => {
        unsafe {
            logger.send_meesage(
                MessageLevel::Panic,
                format_args!($($arg:tt)*)
            )
        }
    };
}



pub const LOG_FILE: &'static str = "logs.txt";


pub static logger: Logger = Logger::new();



pub enum MessageLevel {
    Common,
    Error,
    Panic
}

impl Display for MessageLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(data)
    }
}

pub struct Message {
    level: MessageLevel,
    text: String
}

impl Message {
    pub fn new(level: MessageLevel, text: String) -> Message{
        Self {
            level,
            text
        }
    }
}


pub struct Logger {
    inner: Option<LoggerCore>
}

impl Logger {
    pub const fn new() -> Logger {
        Self {
            inner: None
        }
    }

    pub fn start(&mut self) {
        self.inner = Some(LoggerCore::new())
    }

    pub fn send_meesage(&mut self, level: MessageLevel, text: String) {
        if let Some(core) = &mut self.inner {
            core.send_meesage(level, text);
        }
        else {
            self.start();
            self.send_meesage(level, text);
        }
    }
}


pub struct LoggerCore {
    sender: Sender<Message>,
    thread: Option<JoinHandle<()>>
}

impl LoggerCore {
    fn new() -> LoggerCore {
        let (sender, receiver) = std::sync::mpsc::channel::<Message>();

        let mut log_file = OpenOptions::new()
            .read(true)
            .write(true)
            .append(true)
            .create(true)
            .open(LOG_FILE)
            .unwrap();

        let thread = std::thread::spawn(move || {
            while let Ok(message) = receiver.recv() {
                let mut time = Local::now().format("%d.%m.%Y %H:%M:%S").to_string();

                let log = format!("{} {} {}", time, message.level, message.text);

                let _ = log_file.write_all(log.as_bytes());
            }
        });

        Self {
            sender: sender,
            thread: None
        }
    }

    pub fn send_meesage(&mut self, level: MessageLevel, text: String) {
        
    }
}