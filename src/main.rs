mod server;

use server::Server;

#[derive(clap::Parser)]
pub struct Config {
    #[arg(long, help = "Port on which chat_app will listen for incoming connections")]
    port: u16,
}

#[tokio::main]
async fn main() {
    let config = <Config as clap::Parser>::parse();
    
    match Server::create(config.port).await {
        Ok(mut server) => {
            if let Err(e) = server.run().await {
                println!("Error when starting server: {}", e);
            }
        },
        Err(e) => {
            println!("Couldn't create server: {}", e);
        }

    }
}