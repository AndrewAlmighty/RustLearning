use std::fs::File;
use std::path::PathBuf;
use std::io::{BufRead, Write};

use chrono::{DateTime, FixedOffset};
use rayon::prelude::*;

#[derive(Eq, PartialEq, PartialOrd, Ord)]
struct LogEntry {
    time: DateTime<FixedOffset>,
    entry: String
}

impl LogEntry {
    fn from_line(line: &str) -> Self {
        LogEntry {  time: DateTime::parse_from_rfc3339(line.split_whitespace().nth(0).unwrap()).expect("Unable to create DateTime from log entry"),
                    entry: line.to_string() }
    }
}

pub fn merge_log_files(files: Vec<PathBuf>) {
    let mut all_entries: Vec<LogEntry> = files.par_iter().map(|file| {
                                        extract_entries_from_log(file)
                                        }).flatten().collect();
    all_entries.par_sort_unstable();
    let mut output_file = File::create("merged_log.txt").expect("Failed to create output file");
    all_entries.into_iter().for_each(|entry| output_file.write_fmt(format_args!("{}\n", entry.entry)).expect("Failed to write data to output file") );
}

fn extract_entries_from_log(path: &PathBuf) -> Vec<LogEntry> {
    let f = File::open(path).expect("Unable to open log file");
    let reader = std::io::BufReader::new(f);
    let mut entries = Vec::<LogEntry>::new();
    reader.lines().for_each(|line| entries.push(LogEntry::from_line(&line.unwrap())));
    entries
}



#[test]
fn various_tests() {
    let log_entry = LogEntry::from_line("2025-07-11T08:15:23Z INFO Starting service initialization");
    assert!(log_entry.time.to_rfc2822() == "Fri, 11 Jul 2025 08:15:23 +0000".to_string());
}