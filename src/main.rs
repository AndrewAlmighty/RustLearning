pub mod log;

use crate::log::*;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(clap::Parser)]
pub struct Config {
    #[arg(long, default_value = "info", help = "Logging level. Available levels: debug, info, error.")]
    log_level: String,
    #[arg(long, help = "Path to log file")]
    log_file: Option<PathBuf>
}

fn main() {
    let config = <Config as clap::Parser>::parse();
    let logger;
    match Logger::create(config.log_file, config.log_level) {
        Ok(l) => { logger = l; }
        Err(e) => {
            println!("Error when creating logger: {}", e);
            return;
        }
    }

    let logger_is_running = logger.run();
    log!("app", log::Level::Error, "Testujemy".to_string());
    std::thread::sleep(std::time::Duration::from_secs(1));
    logger_is_running.store(false, Ordering::Relaxed);
}
