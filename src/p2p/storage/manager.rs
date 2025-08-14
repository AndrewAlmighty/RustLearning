use crate::StatusSender;
use crate::reporting::display::LogSender;
use crate::reporting::log;
use crate::p2p::net::message::NodeMessage;
use crate::p2p::storage::message::{StorageMessage, InternalStorageMessage};
use crate::p2p::storage::file::{SharedFile, UNFINISHED_FILE_EXTENSION};

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;

pub(super) type NodeMessageReceiver = tokio::sync::mpsc::UnboundedReceiver<NodeMessage>;
pub(super) type StorageMessageSender = tokio::sync::mpsc::UnboundedSender<StorageMessage>;

pub(super) type InternalMessageSender = tokio::sync::mpsc::UnboundedSender<InternalStorageMessage>;
type InternalMessageReceiver = tokio::sync::mpsc::UnboundedReceiver<InternalStorageMessage>;

const MAX_FILES_TO_SHARE_AT_ONCE: usize = 5;

pub(super) struct Manager {
    log_sender: LogSender,
    storage_status_sender: StatusSender,
    storage_message_sender: StorageMessageSender,
    node_message_receiver: NodeMessageReceiver,
    internal_message_sender: InternalMessageSender,
    internal_message_receiver: InternalMessageReceiver,
    shared_files_dir: PathBuf,
    files: HashMap<String, SharedFile>,
    processing_metadata_for_peers: HashMap<String, HashSet<SocketAddr>>
}

impl Manager {
    pub(super) async fn create(log_sender: LogSender, storage_status_sender: StatusSender, storage_message_sender: StorageMessageSender, node_message_receiver: NodeMessageReceiver,  shared_files_dir: PathBuf) -> Result<Self, std::io::Error> {
        if !shared_files_dir.exists() {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("{:?} does not exists.", shared_files_dir)));
        }
        else if !shared_files_dir.is_dir() {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("{:?} Must points to directory.", shared_files_dir)));
        }

        let (internal_message_sender, internal_message_receiver) = tokio::sync::mpsc::unbounded_channel::<InternalStorageMessage>();

        Ok(Manager { 
            log_sender: log_sender,
            storage_status_sender: storage_status_sender,
            shared_files_dir: shared_files_dir,
            storage_message_sender: storage_message_sender,
            node_message_receiver: node_message_receiver,
            internal_message_sender: internal_message_sender,
            internal_message_receiver: internal_message_receiver,
            files: HashMap::new(),
            processing_metadata_for_peers: HashMap::new()
        })
    }

    pub(super) fn start(mut self) {
        tokio::spawn(async move {
            self.run().await;
        });
    }

    async fn send_current_status(&self) {
        assert!(self.storage_status_sender.send(format!("Files to share in {}:\n{}", self.shared_files_dir.display(), self.files.iter().map(|(_, file)| file.to_string()).collect::<Vec<_>>().join("\n"))).await.is_ok());
    }

    fn log(&self, level: log::Level, text: String) {
        assert!(self.log_sender.send(log::create("manager".to_string(), level, text)).is_ok());
    }

    fn send_message_to_node(&self, message: StorageMessage) {
        self.log(log::Level::Debug, format!("Sending a message to node: {:?}", message));
        if let Err(e) = self.storage_message_sender.send(message) {
            self.log(log::Level::Error, format!("Could not send a message to node: {}", e));
        }
    }

    async fn check_directory(&mut self) {
        self.log(log::Level::Debug, format!("Checking {:?} for any changes", self.shared_files_dir));
        let mut change_detected = false;
        {
            let removed_files = self.files.extract_if(|_, file| !file.file_exists()).collect::<Vec<_>>();
            if !removed_files.is_empty() {
                change_detected = true;
                self.log(log::Level::Info, format!("Files were removed: {:?}", removed_files.iter().map(|el| el.0.as_str()).collect::<Vec<_>>()));
            }

        }

        let items = self.shared_files_dir.read_dir();
        assert!(items.is_ok());
        for item in items.unwrap() {
            match item {
                Err(e) => { self.log(log::Level::Error, format!("Error when reading {:?}: {}", self.shared_files_dir, e)); }
                Ok(item) => {
                    if let Ok(metadata) = item.metadata() {
                        if !metadata.is_file()  {
                            self.log(log::Level::Debug, format!("Found {:?} in {:?}. Expected only files inside directory with shared files. Ignoring it.", item, self.shared_files_dir));
                        }
                        else {
                            let file = item.path();
                            assert!(file.exists());
                            match file.file_name() {
                                None => self.log(log::Level::Error, format!("Discovered a new file: {}, but could not extract it's file name. Ignoring file.", file.display())),
                                Some(file_name) => {
                                    let mut file_name: String = file_name.to_string_lossy().into_owned();
                                    let unfinished;
                                    if file_name.ends_with(UNFINISHED_FILE_EXTENSION) {
                                        file_name.truncate(file_name.len() - UNFINISHED_FILE_EXTENSION.len() - 1);
                                        unfinished = true;
                                    }
                                    else { unfinished = false; }

                                    let file_size = metadata.len();

                                    if !unfinished && file_size == 0 {
                                        self.log(log::Level::Debug, format!("Found {:?} in {:?} and it has size 0. File cannot be empty. Ignoring it.", item, self.shared_files_dir));
                                    }
                                    else if !self.files.contains_key(&file_name) {
                                        if self.files.len() < MAX_FILES_TO_SHARE_AT_ONCE {
                                            if !change_detected { change_detected = true; }
                                            self.log(log::Level::Info, format!("Discovered a new file: {} - going to share it in p2p network.", file_name));
                                            assert!(self.files.insert(file_name.clone(), SharedFile::create_from_existing_file(file_name, file, file_size, unfinished)).is_none());
                                        }
                                        else { self.log(log::Level::Error, format!("Discovered a new file: {}, but reached limit of max shared files at once: {}. Ignoring file.", file_name, MAX_FILES_TO_SHARE_AT_ONCE)); }
                                    }
                                }
                            }
                        }    
                    }
                }
            }
        }
        
        if self.files.len() < MAX_FILES_TO_SHARE_AT_ONCE {
            self.send_message_to_node(StorageMessage::AskForFiles);
        }

        for file in  self.files.values_mut() { if file.update_status_if_needed() { change_detected = true; }}
        if change_detected { self.send_current_status().await; }
    }

    async fn check_files_from_peer(&mut self, peer: SocketAddr, files: Vec<String>) {
        let mut change_detected = false;
        for file in files {
            if let Some(shared_file) = self.files.get(&file) {
                if !shared_file.is_finished() {
                    self.send_message_to_node(StorageMessage::AskPeerForFile(file, peer));
                }
            }
            else {
                if self.files.len() >= MAX_FILES_TO_SHARE_AT_ONCE {
                    self.log(log::Level::Error, format!("Peer: {} has a file: {} which we don't have, but do not have a free slot for it", peer, file));
                    continue;
                }
                else {
                    self.log(log::Level::Info, format!("Peer: {} has a file: {} which we don't have. Asking for details it to our shared files and asking for details.", peer, file));
                    if let Some(shared_file) = SharedFile::create_a_new_file(file.clone(), self.shared_files_dir.clone()) {
                        self.send_message_to_node(StorageMessage::AskPeerForFile(file.clone().to_string(), peer));
                        self.files.insert(file, shared_file);
                        change_detected = true;
                    }
                    else {
                        self.log(log::Level::Error, "Failed to create a new file".to_string());
                    }
                }
            }
        }
        if change_detected { self.send_current_status().await; }
    }

    fn get_metadata_for_peer(&mut self, file_name: String, peer: SocketAddr) {
        if let Some(file) = self.files.get(&file_name) {
            if file.file_exists() { //well user can remove file before app check
                if !file.is_empty_file() {  // this file could be created because we learn of new file, but we are awaiting for metadata too.
                    if let Some(metadata_processing_for) = self.processing_metadata_for_peers.get(&file_name) { 
                        if metadata_processing_for.contains(&peer) { return; }
                    }
                    if let Some(metadata) = file.generate_metadata_for_share(self.internal_message_sender.clone()) {
                        assert!(self.storage_message_sender.send(StorageMessage::SendMetadata(file_name, peer, file.file_size(), metadata)).is_ok());
                    }
                    else {
                        if let Some(peers) = self.processing_metadata_for_peers.get_mut(&file_name) {
                            peers.insert(peer);
                        }
                        else {
                            let mut new_set = HashSet::new();
                            new_set.insert(peer);
                            self.processing_metadata_for_peers.insert(file_name, new_set);
                        }
                    }
                }
            }
        }
        else {
            self.log(log::Level::Error, format!("Peer {} asked for {} which we don't have", peer, file_name));
        }
    }

    fn process_downloads(&mut self) {
        for (_, shared_file) in &mut self.files {
            if shared_file.is_finished() { continue; }
            else if let Some(download_request) = shared_file.get_list_of_requested_chunks() {
                if download_request.len() == 1 {
                    assert!(self.storage_message_sender.send(StorageMessage::AskPeersForFileExcept(shared_file.file_name().to_string(), download_request.keys().map(|peer| peer.clone()).collect())).is_ok());
                }
                for (peer, chunks) in download_request {
                    assert!(self.storage_message_sender.send(StorageMessage::RequestFileChunks(peer, shared_file.file_name().to_string(), chunks)).is_ok());
                }
            }
            else {
                assert!(self.storage_message_sender.send(StorageMessage::AskPeersForFileExcept(shared_file.file_name().to_string(), shared_file.get_source_peers())).is_ok());
            }
        }
    }

    async fn run(&mut self) {
        self.send_current_status().await;
        let mut check_directory_ticker = tokio::time::interval(tokio::time::Duration::from_secs(1));
        loop {
            self.process_downloads();
            tokio::select!(
                _ = check_directory_ticker.tick() => {
                    self.check_directory().await;
                }
                node_message = self.node_message_receiver.recv() => {
                    match node_message {
                        None => { self.log(log::Level::Error, "Could not read a message from node".to_string()); }
                        Some(message) => { match message {
                            NodeMessage::ListFiles(peer) => {
                                self.send_message_to_node(StorageMessage::FilesAvailable(peer, self.files.iter().filter(|(_, file)| !file.is_empty_file()).map(|(_, file)| file.file_name().to_string()).collect()));
                            }
                            NodeMessage::FilesAvailable(peer, files) => {
                                self.check_files_from_peer(peer, files).await;
                            }
                            NodeMessage::AskForFile(file, peer) => {
                                self.get_metadata_for_peer(file, peer);
                            }
                            NodeMessage::ReceivedMetadata(file_name, peer, file_size, metadata) => {
                                if let Some(shared_file) = self.files.get_mut(&file_name) {
                                    if !shared_file.file_exists() {
                                        self.log(log::Level::Debug, format!("Received metadata for file: {} from {}, which is in our set of files, but files doesn't exists. Ignoring it.", file_name, peer));
                                    }
                                    else if shared_file.is_empty_file() == false {
                                        if !shared_file.has_download_metadata() {
                                            self.log(log::Level::Error, format!("Received metadata for file: {} from {}, which is in our set of files and it is completed. Ignoring it.", file_name, peer));
                                        }
                                        else if let shared_file_size = shared_file.file_size() && shared_file_size != file_size {
                                            self.log(log::Level::Debug, format!("Received metadata for file: {} from {}, which is in our set of files, but differs in file size - ours: {}, from peer: {}. Ignoring it.", file_name, peer, shared_file_size, file_size));
                                        }
                                        else if let Some(error) = shared_file.compare_download_metadata(peer, metadata) {
                                            self.log(log::Level::Debug, format!("Received metadata for file: {} from {}, which is in our set of files, but metadata differs: {}. Ignoring it.", file_name, peer, error));
                                        }
                                        else {
                                            self.log(log::Level::Debug, format!("Received metadata for file: {} from {} matches our file. Updated peers info in our file.", file_name, peer));
                                        }
                                    }
                                    else {
                                        shared_file.insert_download_metadata(peer, file_size, metadata);
                                        self.log(log::Level::Debug, format!("Received metadata for file: {} from {}. Appending it to our shared file.", file_name, peer));
                                    }
                                }
                                else {
                                    self.log(log::Level::Debug, format!("Received metadata for file: {} from {}, which is not in our set of files. Ignoring it.", file_name, peer));
                                }
                            }
                            NodeMessage::PeerNotConnected(peer) => {
                                self.log(log::Level::Debug, format!("Received peer: {} not connected message.", peer));
                                for (_, shared_file) in &mut self.files {
                                    shared_file.remove_source_peer(&peer);
                                }
                            }
                            NodeMessage::RequestFileChunks(peer, file_name, chunks) => {
                                self.log(log::Level::Debug, format!("Received Request file chunks for: {} from peer: {}. Chunks requested: {:?}.", file_name, peer, chunks));
                                if let Some(shared_file) = self.files.get_mut(&file_name) {
                                    let chunks_to_send = shared_file.get_file_chunks(chunks);
                                    if !chunks_to_send.is_empty() {
                                        assert!(self.storage_message_sender.send(StorageMessage::SendFileChunks(peer, file_name, chunks_to_send)).is_ok());
                                    }
                                }
                            }
                            NodeMessage::ReceivedFileChunks(file_name, chunks) => {
                                self.log(log::Level::Debug, format!("Received file chunks for: {}", file_name));
                                if let Some(shared_file) = self.files.get_mut(&file_name) {
                                    shared_file.append_chunks_to_file(chunks);
                                    if shared_file.is_finished() {
                                        self.log(log::Level::Info, format!("Finished downloading file: {}.", file_name));
                                    }
                                    self.send_current_status().await;
                                }
                            }
                        }}
                    }
                }
                internal_message = self.internal_message_receiver.recv() => {
                    match internal_message {
                        None => { self.log(log::Level::Error, "Could not read an internal message".to_string()); }
                        Some(message) => {
                            match message {
                                InternalStorageMessage::DownloadMetadataReady(file_name, file_size, metadata) => {
                                    if let Some(peers) = self.processing_metadata_for_peers.remove(&file_name) {
                                        self.log(log::Level::Debug, format!("Processed a metadata for: {}, requested for peers: {:?}.", file_name, peers));
                                        for peer in peers {
                                            assert!(self.storage_message_sender.send(StorageMessage::SendMetadata(file_name.clone(), peer, file_size, metadata.clone())).is_ok());
                                        }
                                    }
                                    else {
                                        self.log(log::Level::Error, format!("Processed a metadata for: {}, but there is no entry of peers awaiting for it.", file_name));
                                    }
                                }
                            }
                        }
                    }
                }
            );
        }
    }
}