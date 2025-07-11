mod merger;

use std::env::args;
use std::path::PathBuf;

fn main() {
    let args = args().skip(1).collect::<Vec<String>>();

    if args.is_empty() {
        println!("Please provide paths to log files. You can pass as arguments many log files");
        return;
    }

    let mut files = Vec::<PathBuf>::with_capacity(args.len());

    for arg in &args {
        let file = PathBuf::from(arg);
        if !file.exists() {
            println!("{} doesn't exists", arg);
            return;
        }
        else if !file.is_file() {
            println!("{} is not a file", arg);
            return;
        }

        files.push(file);
    }

    merger::merge_log_files(files);
}
