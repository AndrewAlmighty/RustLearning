use crate::reporting::{log, log::Log};
use crate::StatusReceiver;

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{stdout, Write};
use std::path::PathBuf; 

pub type LogSender = tokio::sync::mpsc::UnboundedSender<Log>;
pub type LogReceiver = tokio::sync::mpsc::UnboundedReceiver<Log>;

use crossterm::{execute, terminal::{Clear, ClearType}, cursor::MoveTo};

const AMOUNT_OF_LOGS_TO_PRINT: usize = 10;

pub struct Display {
    logs_to_print: BTreeSet<Log>,
    last_node_status_to_print: String,
    last_storage_status_to_print: String,
    log_receiver: LogReceiver,
    node_status_receiver: StatusReceiver,
    storage_status_receiver: StatusReceiver,
    log_file: Option<File>,
    log_level: log::Level
}

impl Display {
    pub fn create(log_receiver: LogReceiver, node_status_receiver: StatusReceiver, storage_status_receiver: StatusReceiver, log_file_path: Option<PathBuf>, log_level: String) -> Result<Self, std::io::Error> {
        let mut log_file = None;
        if let Some(path) = log_file_path {
            match File::create(path) {
                Ok(file) => { log_file = Some(file); }
                Err(e) => { return Err(e); }
            }
        }

        let log_lvl = match log_level.as_str() {
            "debug" => log::Level::Debug,
            "info" => log::Level::Info,
            "error" => log::Level::Error,
            invalid => { return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Invalid logging level: {}. Available: debug, info, error", invalid))); }
        };

        Ok(Display {
            log_receiver: log_receiver,
            node_status_receiver: node_status_receiver,
            storage_status_receiver: storage_status_receiver,
            logs_to_print: BTreeSet::new(),
            log_file: log_file,
            last_node_status_to_print: String::new(),
            last_storage_status_to_print: String::new(),
            log_level: log_lvl
        })
    }

    pub async fn run(&mut self) {
        execute!(stdout(), Clear(ClearType::Purge), MoveTo(0,0)).unwrap();
        loop {
            tokio::select!(
                Some(mut received_log) = self.log_receiver.recv() => {
                    if received_log.get_level() >= self.log_level {
                        if self.logs_to_print.len() >= AMOUNT_OF_LOGS_TO_PRINT {
                            let _ = self.logs_to_print.pop_first();
                        }

                        if let Some(file) = self.log_file.as_mut() {
                            if let Err(e) = file.write_all(format!("{}\n", received_log).as_bytes()) {
                                self.logs_to_print.insert(log::create("display".to_string(), log::Level::Error, format!("Could not save log to file: {}", e.to_string())));
                            }
                            else {
                                let _ = file.flush();
                            }
                        }
                        received_log.truncate();
                        self.logs_to_print.insert(received_log);
                    }
                }
                Some(received_node_status) = self.node_status_receiver.recv() => {
                    self.last_node_status_to_print = received_node_status;
                }
                Some(received_storage_status) = self.storage_status_receiver.recv() => {
                    self.last_storage_status_to_print = received_storage_status;
                }
            );
            execute!(stdout(), Clear(ClearType::All), Clear(ClearType::Purge), MoveTo(0,0)).unwrap();
            self.print();
        }
    }

    fn print(&self) {
        println!("{}\n", self.last_node_status_to_print);
        println!("{}\n", self.last_storage_status_to_print);
        for log in &self.logs_to_print {
            println!("{}", log);
        }
    }
}

