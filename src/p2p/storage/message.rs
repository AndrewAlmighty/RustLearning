use crate::p2p::storage::file::DownloadMetadata;

use std::collections::HashSet;
use std::net::SocketAddr;

#[derive(Debug)]
pub enum StorageMessage {
    AskForFiles,
    FilesAvailable(SocketAddr, Vec<String>),
    AskPeerForFile(String, SocketAddr),
    AskPeersForFileExcept(String, HashSet<SocketAddr>),
    SendMetadata(String, SocketAddr, u64, DownloadMetadata),
    RequestFileChunks(SocketAddr, String, Vec<(usize, u64)>),
    SendFileChunks(SocketAddr, String, Vec<(usize, Vec<u8>)>)
}

#[derive(Debug)]
pub(super) enum InternalStorageMessage {
    DownloadMetadataReady(String, u64, DownloadMetadata)
}