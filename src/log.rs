use chrono::{DateTime, Utc};

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{Sender, Receiver, RecvTimeoutError},
    OnceLock, Arc};

use std::time::Duration;

pub static LOG_SENDER: OnceLock<Sender<Log>> = OnceLock::new();
pub static LOGGING_LEVEL: OnceLock<Level> = OnceLock::new();

#[macro_export]
macro_rules! log {
    ( $module: literal, $lvl: expr, $contents: expr) => {
        if $lvl >= *LOGGING_LEVEL.get().expect("LOGGING_LEVEL is not initialized") {
            match LOG_SENDER.get().expect("LOG_SENDER is not initialized").send(
                Log::create(
                    $module.to_string(),
                    std::path::Path::new(file!()).file_name().unwrap().to_str().unwrap().to_string(),
                    $lvl,
                    $contents)) {
                Ok(()) => {}
                Err(e) => { println!("Could not send a log to logger: {}", e) }
            }
        }
    };
}

pub use crate::log as log;

#[derive(Debug, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Error
}

pub struct Log {
    timestamp: DateTime<Utc>,
    module: String,
    file: String,
    contents: String,
    level: Level,
}

impl Log {
    pub fn create(module: String, file: String, level: Level, contents: String) -> Self {
        Log { timestamp: Utc::now(), module: module, file: file, level: level, contents: contents }
    }
}

impl fmt::Display for Log {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}][{}][{}][{:?}] {}", self.timestamp.to_rfc3339(), self.file, self.module, self.level, self.contents)
    }
}

pub struct Logger {
    logs_receiver: Receiver<Log>,
    logs_file: Option<File>,
}

impl Logger {
    pub fn create(log_file_path: Option<PathBuf>, logging_level: String) -> Result<Self, std::io::Error> {
        match logging_level.as_str() {
            "error" => assert!(LOGGING_LEVEL.set(Level::Error).is_ok()),
            "info" => assert!(LOGGING_LEVEL.set(Level::Info).is_ok()),
            "debug" => assert!(LOGGING_LEVEL.set(Level::Debug).is_ok()),
            unexpected => { return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, format!("Not such logging level: {}", unexpected))); }
        }
        let (tx, rx) = std::sync::mpsc::channel::<Log>();
        assert!(LOG_SENDER.set(tx).is_ok());

        match log_file_path {
            Some(path) => {
                match OpenOptions::new().write(true).create_new(true).open(&path) {
                    Ok(file) => {
                        log!("log", Level::Info, format!("Created log file: {}", path.display()));
                        Ok(Logger{ logs_receiver: rx, logs_file: Some(file)})
                    }
                    Err(e) => Err(e)
                }
            }
            None => Ok(Logger { logs_receiver: rx, logs_file: None})
        }
    }

    pub fn run(mut self) -> Arc<AtomicBool> {
        let running = Arc::new(AtomicBool::new(true));
        let r = Arc::clone(&running);

        std::thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                match self.logs_receiver.recv_timeout(Duration::from_millis(10)) {
                    Ok(log) => {
                        println!("{}", log);
                        if let Some(file) = self.logs_file.as_mut() {
                            if let Err(e) = writeln!(file, "{}", log) {
                                println!("{}", Log::create("Log".to_string(), "log.rs".to_string(), Level::Error, format!("Could not save log to file: {}", e)));
                            }
                            else {
                                let _ = file.flush();
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(e) => { println!("[ERROR] LOGGER FAILURE - CANNOT READ LOGS: {}", e); }
                }
            }
        });

        r
    }
}

#[cfg(test)]
pub mod test_init {
    use super::*;
    use std::sync::Mutex;
    pub fn get_logger() -> &'static Mutex<Logger> {
        static LOGGER: OnceLock<Mutex<Logger>> = OnceLock::new();
        LOGGER.get_or_init(|| {
            Mutex::new(Logger::create(None, "info".to_string()).expect("Logger for tests should be created"))
        })
    }
}

#[test]
#[ignore] // run explicitly, then it will test only logs
fn test_logs() {
    use crate::log::test_init::get_logger;
    let logger = get_logger().lock().expect("There should be a test logger in mutex");
    let current_timestamp = Utc::now();
    log!("test", Level::Error, "something to log".to_string());
    let log_result = logger.logs_receiver.recv_timeout(Duration::from_secs(1));
    assert!(log_result.is_ok());
    let log = log_result.unwrap();
    assert_eq!(log.module.as_str(), "test");
    assert_eq!(log.level, Level::Error);
    assert_eq!(log.contents.as_str(), "something to log");
    assert_eq!(log.file.as_str(), "log.rs");
    assert_eq!(log.timestamp.timestamp(), current_timestamp.timestamp());
    let formatted_log = format!("{}", log);
    let (time, rest) = formatted_log.split_at(37);
    let mut timestamp_pattern = current_timestamp.to_rfc3339();
    timestamp_pattern.truncate(19);
    assert_eq!(time[..20], format!("[{}", timestamp_pattern));
    assert_eq!(rest, "[log.rs][test][Error] something to log");
    log!("test", Level::Info, "something to log - info".to_string());
    assert!(logger.logs_receiver.recv_timeout(Duration::from_secs(1)).is_ok());
    log!("test", Level::Debug, "something to log - debug".to_string());
    assert!(logger.logs_receiver.recv_timeout(Duration::from_secs(1)).is_err());
}