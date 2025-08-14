use crate::p2p::storage::file::DownloadMetadata;

use bincode::{Encode, Decode};

use std::net::SocketAddr;
use std::collections::HashSet;

pub(super) const DISCOVER_HELLO_MESSAGE_SIZE: usize = 3;

#[derive(Encode, Decode, Debug)]
pub(super) struct DiscoverHello {
    pub(super) listening_port: u16
}

#[derive(Encode, Decode, Debug, Clone)]
pub(super) enum NetworkMessage {
    Hello(SocketAddr, bool),
    ConnectionAccepted(Vec<SocketAddr>),
    ConnectionRejected(Vec<SocketAddr>),
    NewPeer(SocketAddr, HashSet<SocketAddr>, HashSet<SocketAddr>),
    ImAlive,
    ListPeers(Vec<SocketAddr>),
    ListFiles(SocketAddr, Option<Vec<String>>),
    AskForFile(String, SocketAddr),
    SendMetadata(String, SocketAddr, u64, DownloadMetadata),
    RequestFileChunks(SocketAddr, String, Vec<(usize, u64)>),
    SendFileChunks(String, Vec<(usize, Vec<u8>)>)
}

#[derive(Debug)]
pub enum NodeMessage {
    ListFiles(SocketAddr),
    FilesAvailable(SocketAddr, Vec<String>),
    AskForFile(String, SocketAddr),
    ReceivedMetadata(String, SocketAddr, u64, DownloadMetadata),
    PeerNotConnected(SocketAddr),
    RequestFileChunks(SocketAddr, String, Vec<(usize, u64)>),
    ReceivedFileChunks(String, Vec<(usize, Vec<u8>)>)
}