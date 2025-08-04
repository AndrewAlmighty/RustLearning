mod client;

#[derive(clap::Parser)]
pub struct Config {
    #[arg(long, help = "Address of chat_app server")]
    server_address: String,
    #[arg(long, help = "Username which will be used for login")]
    username: String,
}

#[tokio::main]
async fn main() {
    let config = <Config as clap::Parser>::parse();
    match client::Client::create(config.server_address, config.username).await {
        Err(e) => { println!("Could not connected to server: {}", e); }
        Ok(client) => {
            client.run().await;
        }
    }
}

