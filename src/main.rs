mod client;
mod server;
mod command;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    if args.len() == 0 {
        println!("Please use --server or --client");
        std::process::exit(1);
    }

    let mut server_mode: Option<bool> = None;

    {
        let mut idx = 0usize;
        for arg in &args {
            match arg.as_str() {
                "--server" => { server_mode = Some(true); args.remove(idx); break; }
                "--client" => { server_mode = Some(false); args.remove(idx); break; }
                _ => {}
            }
            idx += 1;
        }
    }

    match server_mode {
        None => {
            println!("Missing mode. Please use --server or --client");
            std::process::exit(1);
        }

        Some(true) => {
            match server::Server::new(args) {
                Err(msg) => {
                    println!("{}", msg);
                    std::process::exit(1);
                }
                Ok(mut server) => {
                    server.run();
                }
            }
        }

        Some(false) => {
            match client::Client::new(args) {
                Err(msg) => {
                    println!("{}", msg);
                    std::process::exit(1);
                }
                Ok(mut client) => {
                    client.run();
                }
            }
        }
    }
}
