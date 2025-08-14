use chrono::{DateTime, Utc};

use std::cmp::Ordering;
use std::fmt;

type Timestamp = i64;

const MAX_LOG_SIZE_FOR_DISPLAY: usize = 100;

#[derive(Debug, Eq, PartialEq, PartialOrd, Clone, Copy)]
pub enum Level {
    Debug,
    Info,
    Error
}

#[derive(Eq, PartialEq)]
pub struct Log {
    timestamp: Timestamp,
    module: String,
    level: Level,
    content: String
}

impl Log {
    pub(super) fn get_level(&self) -> Level {
        self.level
    }

    pub(super) fn truncate(&mut self) {
        if self.content.len() > MAX_LOG_SIZE_FOR_DISPLAY {
            self.content.truncate(MAX_LOG_SIZE_FOR_DISPLAY);
            self.content.insert_str(MAX_LOG_SIZE_FOR_DISPLAY, " (...)");
        }
    }
}

impl fmt::Display for Log {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match DateTime::from_timestamp_micros(self.timestamp) {
            Some(datetime) => {
                write!(f, "[{}][{}][{:?}] {}", datetime.to_rfc3339(), self.module, self.level, self.content)
            }
            None => Err(fmt::Error{})
        }
    }
}

impl PartialOrd for Log {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.timestamp.partial_cmp(&other.timestamp)
    }
}

impl Ord for Log {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

pub fn create(module: String, level: Level, content: String) -> Log {
    assert!(module.is_empty() == false);
    assert!(content.is_empty() == false);
    Log { timestamp: Utc::now().timestamp_micros(), module: module, level: level, content: content }
}

#[test]
fn test_log() {
    use std::collections::BTreeSet;
    {
        let mut log_set = BTreeSet::<Log>::new();

        log_set.insert(create("main".to_string(), Level::Error, "something happened".to_string()));
        log_set.insert(create("display".to_string(), Level::Info, "display is dead".to_string()));
        log_set.insert(create("node".to_string(), Level::Error, "node is dead".to_string()));
        log_set.insert(create("net".to_string(), Level::Info, "no connection".to_string()));
        log_set.insert(create("display".to_string(), Level::Info, "display is dead".to_string()));
        log_set.insert(create("display".to_string(), Level::Info, "display is dead".to_string()));
        log_set.insert(create("display".to_string(), Level::Info, "display is dead".to_string()));

        let mut last_log = None;
        for log in &log_set {
            match last_log {
                None => last_log = Some(log),
                Some(ll) => assert!(ll < log)
            }
        }
    }
    {
        let mut log = format!("{}", create("main".to_string(), Level::Info, "Something normal".to_string()));
        assert_eq!(log.split_off(34), "[main][Info] Something normal");
        let time_when_log_created = Utc::now();
        log = format!("{}", create("node".to_string(), Level::Error, "Something is no yes".to_string()));
        assert_eq!(log.split_off(34), "[node][Error] Something is no yes");

        let _ = log.pop();
        let _ = log.remove(0);

        let parsed_datetime = DateTime::parse_from_rfc3339(log.as_str());
        assert!(parsed_datetime.is_ok());
        assert_eq!(parsed_datetime.unwrap().timestamp(), time_when_log_created.timestamp());
    }
}