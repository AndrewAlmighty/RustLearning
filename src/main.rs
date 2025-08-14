mod p2p;
mod reporting;

use ipnetwork::IpNetwork;

use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(clap::Parser)]
struct Config {
    #[arg(long, help = "Path to directory where all files will be shared amount p2p network. All directories inside will be ignored.")]
    shared_files_dir: PathBuf,
    #[arg(long, help = "Port on which node will be running and listen for p2p connections.")]
    p2p_port: u16,
    #[arg(long, required = false, conflicts_with_all = ["broadcast_subnet", "broadcast_port"], help = "Directly connect to p2p node, avoid broadcasting hello message")]
    seed_node: Option<SocketAddr>,
    #[arg(long, required = false, help = "path to file where logs file will be saved")]
    log_file: Option<PathBuf>,
    #[arg(long, required = false, conflicts_with = "seed_node", requires = "broadcast_subnet", help = "Port on which node will listen for any hello message")]
    broadcast_port: Option<u16>,
    #[arg(long, required = false, conflicts_with = "seed_node", requires = "broadcast_port", help = "Subnet on which our p2p network will work")]
    broadcast_subnet: Option<IpNetwork>,
    #[arg(long, required = false, default_value = "info", help = "Available levels: debug, info, error")]
    log_level: String
}

type StatusSender = tokio::sync::mpsc::Sender<String>;
type StatusReceiver = tokio::sync::mpsc::Receiver<String>;


#[tokio::main]
async fn main() {
    let config = <Config as clap::Parser>::parse();

    let (log_sender, log_receiver) = tokio::sync::mpsc::unbounded_channel::<reporting::log::Log>();
    let (node_status_sender, node_status_receiver) = tokio::sync::mpsc::channel::<String>(1);
    let (storage_status_sender, storage_status_receiver) = tokio::sync::mpsc::channel::<String>(1);

    match reporting::display::Display::create(log_receiver, node_status_receiver, storage_status_receiver, config.log_file, config.log_level) {
        Ok(mut display) => {
            if let Err(e) = p2p::run(log_sender, node_status_sender, storage_status_sender, config.shared_files_dir, config.p2p_port, config.broadcast_port, config.broadcast_subnet, config.seed_node).await {
                println!("p2p module could not start: {}", e)
            }
            else {
                display.run().await;
            }
        }
        Err(e) => { println!("Could not create a display: {}", e)}
    }
}
