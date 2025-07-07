mod indexer;

use crate::indexer::DirectoryScanner;

use std::path::PathBuf;

fn validate_arguments(mut args: Vec<String>) -> Option<(PathBuf, u8)> {
    if args.len() != 2 {
        println!("App expects two arguments:\nfirst - path to the target directory,\nsecond - number of threads.");
        return None;
    }

    let workers_count: u8;

    if let Some(threads_count) = args.pop() {
        match threads_count.parse::<u32>() {
            Err(arg) => { println!("Argument: {} cannot be parsed to unsigned integer", arg); return None; }
            Ok(arg) => {
                if arg > 16 || arg == 0 {
                    println!("Number of threads must be between 1 - 16");
                    return None;
                }

                workers_count = arg as u8;
            }
        }
    }
    else { return None; }

    match args.pop() {
        None => {
            println!("App expects two arguments, first is path to directory. On its contents some read operations will be performed.");
            return None;
        }

        Some(p) => {
            let path = PathBuf::from(p);
            if !path.exists() {
                println!("Path: {} doesn't exists", path.display());
                return None;
            }
            else if !path.is_dir() {
                println!("Path: {} should point to directory.", path.display());
                return None;
            }

            Some((path, workers_count))
        }
    }
}

fn main() {
    if let Some(args) = validate_arguments(std::env::args().skip(1).collect()) {
        let mut scanner = DirectoryScanner::create(args.0, args.1);
        scanner.run();
    }
}
