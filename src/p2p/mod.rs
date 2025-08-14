mod net;
mod storage;

use ipnetwork::IpNetwork;

use crate::reporting::display::LogSender;
use crate::StatusSender;
use crate::p2p::net::message::NodeMessage;
use crate::p2p::storage::message::StorageMessage;

use std::net::SocketAddr;
use std::path::PathBuf;

pub async fn run(log_sender: LogSender, node_status_sender: StatusSender, storage_status_sender: StatusSender, shared_files_dir: PathBuf, p2p_port: u16, broadcast_port: Option<u16>, broadcast_subnet: Option<IpNetwork>, seed_node: Option<SocketAddr>) -> Result<(), std::io::Error> {
    let (node_message_sender, node_message_receiver) = tokio::sync::mpsc::unbounded_channel::<NodeMessage>();
    let (storage_message_sender, storage_message_receiver) = tokio::sync::mpsc::unbounded_channel::<StorageMessage>();
    net::run(log_sender.clone(), node_status_sender, node_message_sender, storage_message_receiver, p2p_port, broadcast_subnet, broadcast_port, seed_node).await?;
    storage::run(log_sender, storage_status_sender, storage_message_sender, node_message_receiver, shared_files_dir).await?;
    Ok(())
}