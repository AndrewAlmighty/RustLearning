pub(super) mod message;
mod node;
mod connection;

use crate::p2p::net::node::{Node, NodeMessageSender, StorageMessageReceiver};
use crate::reporting::display::LogSender;
use crate::StatusSender;

use ipnetwork::IpNetwork;

use std::net::SocketAddr;

pub async fn run(log_sender: LogSender, node_status_sender: StatusSender, node_message_sender: NodeMessageSender, storage_message_receiver: StorageMessageReceiver, p2p_port: u16, broadcast_subnet: Option<IpNetwork>,  broadcast_port: Option<u16>, seed_node: Option<SocketAddr>) -> Result<(), std::io::Error> {
    let node = Node::create(log_sender.clone(), node_status_sender, node_message_sender, storage_message_receiver, p2p_port, broadcast_subnet, broadcast_port, seed_node).await?;
    node.start();
    Ok(())
}