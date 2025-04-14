use std::io::{self, Write};

pub enum CommandType {
    Cd(std::path::PathBuf),
    Ls(Vec<std::path::PathBuf>),
    Touch(Vec<std::path::PathBuf>),
    Rm(Vec<std::path::PathBuf>),
    Mkdir(Vec<std::path::PathBuf>),
    Read(Vec<std::path::PathBuf>),
    Write((std::path::PathBuf, String)),
    Save(std::path::PathBuf),
    Load(std::path::PathBuf),
    Exit,
}

pub fn print_introduction() {
    println!(r#"
Welcome to basic filesystem simulator.

Available commands:
- ls - show content of specific directory. Default is set to current directory.
- touch - creates a file
- read - prints data from file
- write - adds data to file
- rm - removes file, or EMPTY directory
- mkdir - creates directory
- save - saves current virtual filesystem into single file. This file will be created in real filesystem, not in the one this app simulates.
- load - loads virtual filesystem and overriding current one.  This file will be load from real filesystem, not in the one this app simulates.
- quit - exit app.

Example usages:
ls
ls my_dir my_second_dir
touch nana.txt
touch my_dir/nana.txt
read my_dir/nana.txt
write my_dir/nana.txt "my new data"
rm my_dir/nana.txt
mkdir my_new_dir
save original.vfs
load original.vfs
"#)
}

fn validate_command(input: String) -> Result<CommandType, String> {
    let args:Vec<&str> = input.split_whitespace().collect();

    if !args.is_empty() {
        match args[0] {
            "quit" => { Ok(CommandType::Exit) }
            "cd" => { 
                let args_len = args.len();
                if args_len > 2 {
                    return Err(format!("cd: redundant arguments: {}", args[2..].join(", ")));
                }
                else if args_len < 2 {
                    return Err("cd: missing directory operand".to_string());
                }

                Ok(CommandType::Cd(std::path::PathBuf::from(args[1])))
            }
            "ls" => { Ok(CommandType::Ls(args[1..].iter().map(|s| std::path::PathBuf::from(s)).collect())) }
            "touch" => {
                if args.len() < 2 { Err("touch: missing file operand".to_string()) }
                else { Ok(CommandType::Touch(args[1..].iter().map(|s| std::path::PathBuf::from(s)).collect())) }
            }
            "rm" => {
                if args.len() < 2 { Err("rm: missing file operand".to_string()) }
                else { Ok(CommandType::Rm(args[1..].iter().map(|s| std::path::PathBuf::from(s)).collect())) }
            }
            "mkdir" => {
                if args.len() < 2 { Err("mkdir: missing file operand".to_string()) }
                else { Ok(CommandType::Mkdir(args[1..].iter().map(|s| std::path::PathBuf::from(s)).collect())) }
            }
            "read" => {
                if args.len() < 2 { Err("read: missing file operand".to_string()) }
                else { Ok(CommandType::Read(args[1..].iter().map(|s| std::path::PathBuf::from(s)).collect())) }
            }
            "write" => {
                match args.len() {
                    0 => { Err("args.len() is 0. Suspicious, this should never happen!".to_string()) }
                    1 => { Err("write: missing file operand".to_string()) }
                    2 => { Err("write: missing data to write".to_string()) }
                    3 => {
                        let mut chars = args[2].chars();
                        let first_char = chars.next();
                        let last_char = chars.last();
                        if args[2].len() <= 2 || first_char != Some('\"') || last_char != Some('\"') {
                            return Err("write: data should be in \"\" at has at least one character".to_string());
                        }

                        Ok(CommandType::Write((std::path::PathBuf::from(args[1]), args[2].to_string())))
                    }
                    4.. => { Err(format!("write: redundant arguments: {}", args[3..].join(", "))) }
                }
            }
            "save" => {
                let args_len = args.len();
                if args_len > 2 {
                    return Err(format!("save: redundant arguments: {}", args[2..].join(", ")));
                }
                else if args_len < 2 {
                    return Err("save: missing file operand".to_string());
                }

                Ok(CommandType::Save(std::path::PathBuf::from(args[1]))) 
            }
            "load" => {
                let args_len = args.len();
                if args_len > 2 {
                    return Err(format!("load: redundant arguments: {}", args[2..].join(", ")));
                }
                else if args_len < 2 {
                    return Err("load: missing file operand".to_string());
                }

                Ok(CommandType::Load(std::path::PathBuf::from(args[1])))
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

#[test]
fn check_validate_commands() {
    assert!(validate_command("sdf".to_string()).is_err());
    assert!(validate_command("".to_string()).is_err());
    assert!(validate_command("cd".to_string()).is_err());
    assert!(validate_command("cd asdf qwer".to_string()).is_err());
    assert!(validate_command("touch".to_string()).is_err());
    assert!(validate_command("rm".to_string()).is_err());
    assert!(validate_command("mkdir".to_string()).is_err());
    assert!(validate_command("read".to_string()).is_err());
    assert!(validate_command("write".to_string()).is_err());
    assert!(validate_command("write file asfd".to_string()).is_err());
    assert!(validate_command("write file asfd\"".to_string()).is_err());
    assert!(validate_command("write file \"asfd".to_string()).is_err());
    assert!(validate_command("write file \"asfd\" \"df\"".to_string()).is_err());
    assert!(validate_command("write file \"asfd\" bb".to_string()).is_err());
    assert!(validate_command("save".to_string()).is_err());
    assert!(validate_command("save asfd qrrew".to_string()).is_err());
    assert!(validate_command("load".to_string()).is_err());
    assert!(validate_command("load dff qewqe".to_string()).is_err());

    assert!(matches!(validate_command("quit".to_string()).unwrap(),  CommandType::Exit));
    assert!(matches!(validate_command("cd afsd".to_string()).unwrap(), CommandType::Cd(_)));
    assert!(matches!(validate_command("ls".to_string()).unwrap(), CommandType::Ls(_)));
    assert!(matches!(validate_command("ls asdf".to_string()).unwrap(), CommandType::Ls(_)));
    assert!(matches!(validate_command("ls qwer asdf".to_string()).unwrap(), CommandType::Ls(_)));
    assert!(matches!(validate_command("touch asdf".to_string()).unwrap(), CommandType::Touch(_)));
    assert!(matches!(validate_command("touch qwer asdf".to_string()).unwrap(), CommandType::Touch(_)));
    assert!(matches!(validate_command("rm asdf".to_string()).unwrap(), CommandType::Rm(_)));
    assert!(matches!(validate_command("rm qwer asdf".to_string()).unwrap(), CommandType::Rm(_)));
    assert!(matches!(validate_command("mkdir asdf".to_string()).unwrap(), CommandType::Mkdir(_)));
    assert!(matches!(validate_command("mkdir qwer asdf".to_string()).unwrap(), CommandType::Mkdir(_)));
    assert!(matches!(validate_command("read asdf".to_string()).unwrap(), CommandType::Read(_)));
    assert!(matches!(validate_command("read qwer asdf".to_string()).unwrap(), CommandType::Read(_)));
    assert!(matches!(validate_command("read qwer asdf".to_string()).unwrap(), CommandType::Read(_)));
    assert!(matches!(validate_command("write qwer \"asdf\"".to_string()).unwrap(), CommandType::Write(_)));
    assert!(matches!(validate_command("save qwer ".to_string()).unwrap(), CommandType::Save(_)));
    assert!(matches!(validate_command("load qwer ".to_string()).unwrap(), CommandType::Load(_)));
}