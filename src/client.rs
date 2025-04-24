use crate::command;

use command::Command;
use std::net::{TcpStream, Shutdown};
use std::io::{Write, Read};

fn wait_for_command() -> Command {
    loop {
        std::io::stdout().flush().expect("Failed to flush stdout");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).expect("Failed to read line");

        match input.trim() {
            "INC" => { return Command::INC; }
            "DEC" => { return Command::DEC; }
            "GET" => { return Command::GET; }
            "EXIT" => { return Command::EXIT; }
            wrong_cmd => { println!("Unrecognized command: {}", wrong_cmd); }
        }
    }
}

pub struct Client {
    server: TcpStream
}

impl Client {
    pub fn new(mut args: Vec<String>) -> Result<Self, String> {
        let addr;
        match args.pop() {
            None => { return Err("Client requires server address like 127.0.0.1:80 to which will try to connect".to_string()); }
            Some(a) => { addr = a; }
        }
        println!("Connecting to: {}", addr);
        match TcpStream::connect(addr) {
            Ok(s) => {
                println!("Connected!\nAvailable commands:\nGET - get counter\nDEC - decrease counter by 1\nINC - increase counter by 1\nEXIT - shutdown app");
                return Ok(Client{ server: s });
            }
            Err(e) => {return Err(e.to_string()); }
        }
    }

    pub fn run(&mut self) {
        let mut connection_lost = false;

        while !connection_lost {
            let cmd = wait_for_command();
            
            match cmd {
                Command::EXIT => { return; }
                c => { connection_lost = self.server.write_all(&[c as u8]).is_err(); }
            }

            if !connection_lost {
                let mut buffer = [0u8; 1024];
                match self.server.read(&mut buffer) {
                    Ok(n) => { 
                        if n > 0 {
                            println!("Response: {}", String::from_utf8_lossy(&buffer[..n])); 
                        }
                        else { connection_lost = true; }
                    }
                    Err(_) => connection_lost = true,
                }
            }
        }

        if connection_lost {
            println!("Lost connection to server! Exiting.");
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Ok(remote_addr) = self.server.peer_addr() {
            println!("Closing connection to {}", remote_addr);
            let _ = self.server.shutdown(Shutdown::Both);
        }
    }
}