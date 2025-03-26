mod storage;

use std::io::{self, Write};

fn print_help() {
    println!("Usage:
              -h --help - print help
              -i <path to file> - open existing db or create a new one with desired name");
}

fn run(file: Option<String>) {
    println!("Welcome to storage. Available actions:\n
            - insert - add a new key, value to storage\n
            - get - get a value with given key\n
            - list - list all elements in storage\n
            - delete - remove an item from storage\n
            - quit - exit and saves storage into db.");

    let mut storage: storage::Storage;

    match file {
        None => { storage = storage::Storage::new(); }
        Some(f) => { storage = storage::Storage::from_db(f.as_str()); }
    }

    loop {
        println!("Action: ");
        io::stdout().flush().expect("Failed to flush stdout");
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");
        input = input.trim().to_string();

        match input.as_str(){
            "quit" => { break; }
            "insert" => {
                input.clear();
                println!("Pass a key:");
                io::stdin().read_line(&mut input).expect("Failed to read line");
                input = input.trim().to_string();
                let k = input.clone();
                input.clear();
                println!("Pass a value:");
                io::stdin().read_line(&mut input).expect("Failed to read line");
                input = input.trim().to_string();
                let result = storage.insert(k, input);
                if result.is_err() {
                    println!("{:?}", result);
                }
            }
            "get" => {
                println!("Give me a key: ");
                input.clear();
                io::stdin().read_line(&mut input).expect("Failed to read line");
                input = input.trim().to_string();
                let result = storage.get(input.clone());
                match result {
                    Ok(r) => {
                        println!("Found key: {}, text: {}", input, r);
                    }
                    Err(e) => {
                        println!("{}", e);
                    }
                }
            }
            "list" => {
                println!("Items in storage:\n");
                for el in  storage.list() {
                    println!("{}:{}", el.0, el.1);
                }
            }
            "delete" => {
                input.clear();
                println!("Pass a key:");
                io::stdin().read_line(&mut input).expect("Failed to read line");
                input = input.trim().to_string();
                let result = storage.delete(input.clone());
                match result {
                    Ok(()) => {
                        println!("Deleted an entry with key: {}", input);
                    }
                    Err(e) => {
                        println!("{}", e);
                    }
                }
            }
            &_ => {}
        }
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        run(None);
    }
    else {
        match args[0].as_str() {
            "-h" | "--help" => { print_help(); }
            "-i" => {
                if args.len() == 2 {
                    run(args.pop());
                }
                else { print_help(); }
            }
            _ => { print_help(); }
        }
    }
}
