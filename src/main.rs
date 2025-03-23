use std::collections::HashSet;

use std::env;
use std::fs::File;
use std::path::PathBuf;
use std::io::{Error, ErrorKind, BufRead, BufReader};

fn validate_files_names(args: Vec<String>) -> std::io::Result<Vec<PathBuf>> {
    if args.is_empty() {
        let err_msg = format!("You need to provide at least one path to file");
        return Err(Error::new(ErrorKind::InvalidInput, err_msg));
    }

    let supported_formats: HashSet<&str> = ["md", "txt", "json", "csv"].iter().cloned().collect();
    let mut files: Vec<PathBuf> = Vec::<PathBuf>::with_capacity(args.len());

    for arg in args {
        if arg.is_empty() {
            return Err(Error::new(ErrorKind::InvalidInput, "Passed an empty string to argument list"));
        }
        let path = PathBuf::from(arg);
        if !path.exists() {
            let err_msg = format!("File {:?} doesn't exists", path.to_str());
            return Err(Error::new(ErrorKind::InvalidInput, err_msg));
        }
        else if !path.is_file() {
            let err_msg = format!("Path {:?} doesn't point to a file", path.to_str());
            return Err(Error::new(ErrorKind::InvalidInput, err_msg));
        }

        let ext = path.extension().and_then(|e| e.to_str());

        match ext {
            None => {
                let err_msg = format!("Path {:?} points to file without extension, which means it's probably a binary file.", path.to_str());
                return Err(Error::new(ErrorKind::InvalidInput, err_msg));
            }
            Some(ext) if !supported_formats.contains(ext) => {
                let err_msg = format!("Path {:?} points to file which format is not supported. Supported formats: {:?}.", path.to_str(), supported_formats);
                return Err(Error::new(ErrorKind::InvalidInput, err_msg));
            }

            _ => { files.push(path); }
        }
    }

    Ok(files)
}

fn calculate_lines(files:Vec<PathBuf>) -> std::io::Result<usize> {
    let mut total_lines: usize = 0;

    for file_path in files {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        total_lines += reader.lines().count();
    }

    Ok(total_lines)
}

fn main() {
    let files:Vec<PathBuf> = validate_files_names(env::args().skip(1).collect()).expect("One or more files are not valid or doesn't exists.");
    println!("Sum of lines in passed files: {}", calculate_lines(files).expect("At least one file couldn't be processed successfully."))
}

#[test]
fn test_validate_files_names_bad_examples() {
    let bad_examples = vec![
        Vec::<&str>::new(),
        vec![""],
        vec!["a"],
        vec!["c.doc"]
    ];

    for example in bad_examples {
        let result = validate_files_names(example.iter().map(|s| s.to_string()).collect());
        assert!(result.is_err() == true);
    }
}