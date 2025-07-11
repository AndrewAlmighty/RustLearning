use std::io::{self, Write};

pub enum CommandType {
    Create(String, u32, Option<String>), // Alarm title, timeout in seconds, Message
    Repeat(String), // Alarm Title
    Delete(String), // Alarm Title
}

pub fn print_help() {
    println!(r#"
Timer scheduler commands:
create <title> <timeout in seconds> <message> - create a new alarm, which will be printed after timeout. Message is optional. Example - create budzik 10 pora wstawac
repeat <title> - repeat already set alarm, if it was activated. Example - repeat budzik
delete <title> - delete alarm from db (if it was not activated, it will not be activated). Example = delete budzik
exit - stops app.

"#)
}

pub fn wait_for_command() -> Option<CommandType> {
    loop {
        print!("$ ");
        io::stdout().flush().expect("Failed to flush stdout");
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let args: Vec<&str> = input.trim().split_whitespace().collect();

        if args.is_empty() {
            print_help();
        }
        else {
            let args_count = args.len();
    
            match args[0] {
                "create" => {                    
                    if args_count < 3 {
                        print_help();
                    }
                    else {
                        match args[2].parse::<u32>() {
                            Ok(0) => { println!("Timeout arg is wrong!"); }
                            Ok(timeout) => {
                                let mut message: Option<String> = None;
                                if args_count > 3 {
                                    message = Some(args[3..].join(" "));
                                }                            
                                return Some(CommandType::Create(args[1].to_string(), timeout, message));
                            }
                            Err(_) => { println!("Timeout arg is wrong!"); }
                        }
                    }
                }
                "repeat" => {
                    if args_count != 2 {
                        print_help();
                    }
                    else {
                        return Some(CommandType::Repeat(args[1].to_string()));
                    }
                }
                "delete" => {
                        if args_count != 2 {
                        print_help();
                    }
                    else {
                        return Some(CommandType::Delete(args[1].to_string()));
                    }
                }
                "exit" => { return None; }
                _ => { print_help(); }
            }
        }
    }
}