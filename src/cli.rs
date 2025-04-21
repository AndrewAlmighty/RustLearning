use std::io::{self, Write};

pub enum CommandType {
    AddContact(String, String, String, String), /* name, phone, email, address */
    UpdateContact(String, Option<String>, Option<String>, Option<String>, Option<String>), /* name_or_email, address, email, phone, name */
    RemoveContact(String), // name
    SearchContact(String), // name
    Exit,
}

pub fn print_introduction() {
    println!(r#"
Welcome to basic contacts manager

Available commands:
add name phone number email address
update name_or_email name phone number email address - first argument help finding contact, in next arguments, passing "" instead of new arguments will not update specific field of contact
search name
remove name
quit
"#)
}

fn validate_command(input: String) -> Result<CommandType, String> {
    let args:Vec<&str> = input.split_whitespace().collect();

    if !args.is_empty() {
        match args[0] {
            "quit" => { Ok(CommandType::Exit) }
            "add" => { 
                let args_len = args.len();
                if args_len > 5 {
                    return Err(format!("add: redundant arguments: {}", args[5..].join(", ")));
                }
                else if args_len < 5 {
                    return Err("add: not all arguments provided. Must provide in order: name, phone number, email, address".to_string());
                }

                Ok(CommandType::AddContact(args[1].to_string(), args[2].to_string(), args[3].to_string(), args[4].to_string()))
            }
            "update" => { 
                let args_len = args.len();
                if args_len > 6 {
                    return Err(format!("update: redundant arguments: {}", args[5..].join(", ")));
                }
                else if args_len < 6 {
                    return Err("update: not all arguments provided. Must provide in order: name or email name, phone number, email, address".to_string());
                }

                let mut update_args = Vec::<Option<String>>::with_capacity(4);

                for arg in &args[1..] {
                    if *arg == "\"\"" {
                        update_args.push(None);
                    }
                    else {
                        update_args.push(Some(arg.to_string()));
                    }
                }

                Ok(CommandType::UpdateContact(args[1].to_string(), update_args.pop().expect("address update arg expected"), update_args.pop().expect("email update arg expected"), update_args.pop().expect("phone update arg expected"), update_args.pop().expect("name update arg expected")))
            }
            "search" => { 
                if args.len() != 2 {
                    return Err(format!("search accepts only one argument: name"));
                }

                Ok(CommandType::SearchContact(args[1].to_string()))
            }
            "remove" => { 
                if args.len() != 2 {
                    return Err(format!("remove accepts only one argument: name"));
                }

                Ok(CommandType::RemoveContact(args[1].to_string()))
            }
            unknown => { Err(format!("Unknown command: {}", unknown)) }
        }
    }
    else { Err("Need a command".to_string()) }
}

pub fn wait_for_command() -> Option<CommandType> {
    loop {
        print!("$ ");
        io::stdout().flush().expect("Failed to flush stdout");
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let result = validate_command(input.trim().to_string());

        match result {
            Ok(CommandType::Exit) => { return None; }
            Ok(cmd) => { return Some(cmd); }
            Err(msg) => { println!("{}", msg); }
        }
    }
}