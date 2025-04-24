use crate::command;

use command::Command;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, atomic::{AtomicIsize, Ordering}};
use std::thread;


pub struct Server {
    listener: TcpListener,
    counter: Arc<AtomicIsize>,
    threads: Vec<thread::JoinHandle<()>>
}

impl Server {
    pub fn new(mut args: Vec<String>) -> Result<Self, String> {
        let listener;

        match args.pop() {
            None => { return Err("server requires address, like 127.0.0.1:80".to_string()); }
            Some(addr) => { listener = TcpListener::bind(addr); }
        }
        
        match listener {
            Err(msg) => Err(format!("{}", msg)),
            Ok(l) => {
                println!("Running server on address: {}", l.local_addr().unwrap());
                return Ok(Server{listener: l, counter: Arc::new(AtomicIsize::new(0)), threads: Vec::new() });
            }
        }
    }

    pub fn run(&mut self) {
        for new_connection in self.listener.incoming() {
            match new_connection {
                Ok(connection) => {
                    let counter = Arc::clone(&self.counter);
                    let handler = thread::spawn(move || Server::handle_connection(connection, counter));
                    self.threads.push(handler);
                }
                Err(e) => { println!("Connection failed: {}", e); }
            }
        }

        for t in self.threads.drain(..) {
            match t.join() {
                Ok(()) => {}
                Err(_) => { println!("Error during joining threads"); }
            }
        }
    }

    fn handle_connection(mut tcp_stream: TcpStream, counter: Arc<AtomicIsize>) {
        let client_address;
        match tcp_stream.peer_addr() {
            Ok(addr) => { client_address = addr; println!("New client connected! Address: {}", client_address); }
            Err(e) => { println!("Error when trying to extract client address: {}. New client will not be handled.", e); return; }
        }

        loop {
            let mut buffer = [0u8; 1024];
            match tcp_stream.read(&mut buffer) {
                Ok(0) => { println!("Client with addr: {} disconnected", client_address); break; }
                Err(e) => { println!("Closing connection to client with addr: {} due to error: {}", client_address, e); break; }
                Ok(1) => {
                    let command;
                    match buffer[0] {
                        10 => { command = Command::INC; }
                        11 => { command = Command::DEC; }
                        12 => { command = Command::GET; }
                        v => { println!("Received value that cannot be converted to command: {}", v); command = Command::EXIT; }
                    }

                    let return_message;
                    match command {
                        Command::INC => { 
                            counter.fetch_add(1, Ordering::Relaxed);
                            return_message = Vec::from(b"OK");
                            println!("Client: {} increased counter", client_address);
                        }
                        Command::DEC => {
                            counter.fetch_sub(1, Ordering::Relaxed);
                            return_message = Vec::from(b"OK");
                            println!("Client: {} decreased counter", client_address);
                        }
                        Command::GET => {
                            return_message = Vec::from(format!("Counter: {}", counter.load(Ordering::Relaxed)).as_bytes());
                        }
                        Command::EXIT => { return_message = Vec::from(b"Error - EXIT COMMAND SHOULD NEVER REACH SERVER"); } // this one should never be here
                    }

                    if let Err(e) = tcp_stream.write(&return_message) {
                        println!("Closing connection to client with addr: {} due to error when sending acknowledge message: {}", client_address, e);
                        break;
                    }
                }
                Ok(n) => { println!("Received message from client: {} that cannot be handled: {}", client_address, String::from_utf8_lossy(&buffer[..n])); }

            }
        }
    }
}