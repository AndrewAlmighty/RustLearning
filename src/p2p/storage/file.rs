use crate::p2p::storage::manager::InternalMessageSender;
use crate::p2p::storage::message::InternalStorageMessage;

use bincode::{Encode, Decode};

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::fmt::{Display, Formatter};
use std::io::{Read, Write, Seek, SeekFrom};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Instant, Duration};

type HashType = [u8; 32];

pub(super) const UNFINISHED_FILE_EXTENSION: &str = "unfinished";
const FILE_PIECE_SIZE: usize = 262144; // 256 KiB
const MAX_CHUNKS_REQUESTED_AT_ONCE: usize = 10;
const CHUNK_REQUEST_TIMEOUT_SECONDS: u64 = 20;
const IDLE_TIME_UNTIL_STATUS_CHANGE_SECONDS: u64 = 15;

#[derive(PartialEq)]
enum Status {
    Seeding,
    Peering,
    Idle
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct FileChunk {
    position: u64,
    hash: HashType,
    downloaded: bool
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct DownloadMetadata {
    file_chunks: Vec<FileChunk>,
    file_hash: HashType,
    chunks_downloaded: usize
}

pub(super) struct SharedFile {
    file_name: String,
    file_path: PathBuf,
    file_size: u64,
    download_metadata: Option<DownloadMetadata>,
    peers_with_available_chunks: HashMap<SocketAddr, HashSet<usize>>,
    chunks_requested: HashMap<usize, Instant>,
    last_peers_asked_for_chunks: HashSet<SocketAddr>,
    status_last_updated_time: Instant,
    status: Status,
    file_completion: u8
}

impl SharedFile {
    pub(super) fn create_from_existing_file(file_name: String, file_path: PathBuf, file_size: u64, unfinished: bool) -> Self {
        if !unfinished {
            assert_ne!(file_path.extension().unwrap(), UNFINISHED_FILE_EXTENSION);
            SharedFile {file_name: file_name, file_path: file_path, status: Status::Idle, file_size: file_size, file_completion: 100, download_metadata: None, peers_with_available_chunks: HashMap::new(), chunks_requested: HashMap::new(), status_last_updated_time: Instant::now(), last_peers_asked_for_chunks: HashSet::new() }
        }
        else if file_size == 0 {
            SharedFile {file_name: file_name, file_path: file_path, status: Status::Idle, file_size: file_size, file_completion: 0, download_metadata: None, peers_with_available_chunks: HashMap::new(), chunks_requested: HashMap::new(), status_last_updated_time: Instant::now(), last_peers_asked_for_chunks: HashSet::new() }
        }
        else {
            assert_eq!(file_path.extension().unwrap(), UNFINISHED_FILE_EXTENSION);
            if let Ok(mut file) = File::open(&file_path) {
                assert!(file.seek(SeekFrom::End(-8)).is_ok());
                let mut file_size_buf = [0u8; 8];
                assert!(file.read_exact(&mut file_size_buf).is_ok());
                let file_size_without_metadata = u64::from_le_bytes(file_size_buf);
                assert!(file.seek(SeekFrom::Start(file_size_without_metadata)).is_ok());
                let mut metadata_buf = vec![0u8; (file_size - file_size_without_metadata) as usize];
                assert!(file.read_exact(&mut metadata_buf).is_ok());
                if let Ok((download_metadata, _)) = bincode::decode_from_slice::<DownloadMetadata, bincode::config::Configuration>(&metadata_buf, bincode::config::standard()) {
                    let file_completion = 
                        if download_metadata.chunks_downloaded == 0 { 0u8 }
                        else {
                            let d = download_metadata.chunks_downloaded as f64 / download_metadata.file_chunks.len() as f64;
                            let r = d * 100.0;
                            r.floor() as u8
                        };
                    assert!(file_completion < 100);
                    SharedFile {file_name: file_name, file_path: file_path, status: Status::Idle, file_size: file_size_without_metadata, file_completion: file_completion, download_metadata: Some(download_metadata), peers_with_available_chunks: HashMap::new(), chunks_requested: HashMap::new(), status_last_updated_time: Instant::now(), last_peers_asked_for_chunks: HashSet::new() }
                }
                else {
                    panic!("Could not decode a download metadata from an existing file: {}", file_path.display());
                }  
            }
            else {
                panic!("Could not open an existing file: {}", file_path.display());
            }
        }
    }

    pub(super) fn create_a_new_file(file_name: String, files_directory: PathBuf) -> Option<Self> {
        let mut file_path = files_directory;
        file_path.extend([format!("{}.{}", file_name.clone(), UNFINISHED_FILE_EXTENSION)]);
        match File::create_new(file_path.clone()) {
            Err(_) => None,
            Ok(_) => {
                Some(SharedFile {file_name: file_name, file_path: file_path, status: Status::Idle, file_size: 0, file_completion: 0, download_metadata: None, peers_with_available_chunks: HashMap::new(), chunks_requested: HashMap::new(), status_last_updated_time: Instant::now(), last_peers_asked_for_chunks: HashSet::new() })
            }
        }
    }

    pub(super) fn generate_metadata_for_share(&self, internal_message_sender: InternalMessageSender) -> Option<DownloadMetadata> {
        if let Some(download_metadata) = &self.download_metadata {
            Some(download_metadata.clone())
        }
        else {
            assert!(self.file_completion == 100);
            let file_path = self.file_path.clone();
            let file_name = self.file_name.clone();
            let file_size = self.file_size;

            tokio::spawn(async move {
                let mut file = File::open(&file_path).unwrap();
                let mut chunks_count = (file_size / FILE_PIECE_SIZE as u64) as usize;
                if file_size % FILE_PIECE_SIZE as u64 != 0 { chunks_count += 1; }
                let mut file_chunks_metadata = Vec::<FileChunk>::with_capacity(chunks_count);
                let mut file_hasher = blake3::Hasher::new();
                let mut current_position = 0u64;
                while current_position < file_size {
                    let mut chunk_hasher = blake3::Hasher::new();
                    let mut buf =
                        if file_size < current_position + FILE_PIECE_SIZE as u64 { vec![0u8; (file_size - current_position) as usize] }
                        else {vec![0u8; FILE_PIECE_SIZE] };

                    if file.read_exact(&mut buf).is_err() { break; }
                    chunk_hasher.update(&buf);
                    file_hasher.update(&buf);
                    let chunk = FileChunk { position: current_position, hash: *chunk_hasher.finalize().as_bytes(), downloaded: true };
                    file_chunks_metadata.push(chunk);
                    chunks_count -= 1;
                    current_position += buf.len() as u64;
                }

                assert!(chunks_count == 0);
                assert!(internal_message_sender.send(InternalStorageMessage::DownloadMetadataReady(
                    file_name,
                    file_size,
                    DownloadMetadata {file_hash: *file_hasher.finalize().as_bytes(), file_chunks: file_chunks_metadata, chunks_downloaded: 0 }
                )).is_ok());
            });

            None
        }
    }

    pub(super) fn insert_download_metadata(&mut self, peer: SocketAddr, file_size: u64, mut download_metadata: DownloadMetadata) {
        assert!(self.download_metadata.is_none());
        assert!(self.file_size == 0);
        assert!(self.file_path.exists());
        
        if let Ok(mut file) = OpenOptions::new().write(true).open(&self.file_path) {
            {
                let metadata = file.metadata();
                assert!(metadata.is_ok());
                assert!(metadata.unwrap().len() == 0);
            }
            self.file_size = file_size;
            assert!(file.set_len(file_size).is_ok());
            assert!(file.seek(SeekFrom::End(0)).is_ok());
            let encoded = bincode::encode_to_vec(&download_metadata, bincode::config::standard()).unwrap();
            assert!(file.write_all(&encoded).is_ok());
            assert!(file.write_all(&self.file_size.to_le_bytes()).is_ok());
        }
        else {
            panic!("Cannot open file: {}", self.file_path.display());
        }

        let mut peers_available_chunks = HashSet::<usize>::new();
        for i in 0..download_metadata.file_chunks.len() {
            if download_metadata.file_chunks[i].downloaded { peers_available_chunks.insert(i); }
            download_metadata.file_chunks[i].downloaded = false;
        }

        download_metadata.chunks_downloaded = 0;
        self.peers_with_available_chunks.insert(peer, peers_available_chunks);
        self.download_metadata = Some(download_metadata);
    }

    pub(super) fn flush_download_metadata(&self) {
        assert!(self.download_metadata.is_some());
        assert!(self.file_size != 0);
        assert!(self.file_path.exists());
        if let Ok(mut file) = OpenOptions::new().write(true).open(&self.file_path) {
            if let Some(download_metadata) = &self.download_metadata {
                assert!(file.set_len(self.file_size).is_ok());
                assert!(file.seek(SeekFrom::End(0)).is_ok());
                let encoded = bincode::encode_to_vec(&download_metadata, bincode::config::standard()).unwrap();
                assert!(file.write_all(&encoded).is_ok());
                assert!(file.write_all(&self.file_size.to_le_bytes()).is_ok());
            }
        }
    }

    pub(super) fn compare_download_metadata(&mut self, peer: SocketAddr, peer_download_metadata: DownloadMetadata) -> Option<String> {
        assert!(self.download_metadata.is_some());

        if let Some(our_download_metadata) = &self.download_metadata {
            let chunks_count = our_download_metadata.file_chunks.len();
            if peer_download_metadata.file_chunks.len() != chunks_count {  return Some(format!("Peer's metadata chunks count: {}, our file chunks count: {}", peer_download_metadata.file_chunks.len(), chunks_count )); }
            for i in 0..chunks_count {
                if peer_download_metadata.file_chunks[i].hash != our_download_metadata.file_chunks[i].hash { return Some(format!("chunk: {}, peer's chunk hash: {:?}, our chunk hash: {:?}", i, peer_download_metadata.file_chunks[i].hash, our_download_metadata.file_chunks[i].hash )); }
                if peer_download_metadata.file_chunks[i].position != our_download_metadata.file_chunks[i].position { return Some(format!("chunk: {}, peer's chunk position: {}, our chunk position: {}", i, peer_download_metadata.file_chunks[i].position, our_download_metadata.file_chunks[i].position )); }
            }
            if peer_download_metadata.file_hash != our_download_metadata.file_hash { return Some(format!("Peer's metadata file hash: {:?}, our file hash: {:?}", peer_download_metadata.file_hash, our_download_metadata.file_hash )); }
            let mut peers_available_chunks = HashSet::<usize>::new();
            for i in 0..peer_download_metadata.file_chunks.len() {
                if peer_download_metadata.file_chunks[i].downloaded && !our_download_metadata.file_chunks[i].downloaded { peers_available_chunks.insert(i); }
            }
            self.peers_with_available_chunks.insert(peer, peers_available_chunks);
        }
        None
    }

    pub(super) fn get_list_of_requested_chunks(&mut self) -> Option<HashMap<SocketAddr, Vec<(usize, u64)>>> {
        if let Some(download_metadata) = &self.download_metadata {
            let _ = self.chunks_requested.extract_if(|_, asked_time| asked_time.elapsed() >= Duration::from_secs(CHUNK_REQUEST_TIMEOUT_SECONDS)).collect::<Vec<_>>();
            let _ = self.peers_with_available_chunks.extract_if(|_, idxs| idxs.is_empty()).collect::<Vec<_>>();
            if self.chunks_requested.len() >= MAX_CHUNKS_REQUESTED_AT_ONCE { return None; }
            let mut request_map = HashMap::<SocketAddr, Vec<(usize, u64)>>::new();
            let mut available_chunks_requests = MAX_CHUNKS_REQUESTED_AT_ONCE - self.chunks_requested.len();
            let max_possible_chunks_requests = download_metadata.file_chunks.len() - download_metadata.chunks_downloaded;

            if available_chunks_requests < self.peers_with_available_chunks.len() {
                return Some(HashMap::new());
            }
            
            while 
                available_chunks_requests > 0 &&
                max_possible_chunks_requests > self.chunks_requested.len() &&
                self.peers_with_available_chunks.len() > 0 {
                for (peer, available_chunks) in &self.peers_with_available_chunks {
                    if available_chunks_requests == 0 { break; }
                    for chunk_index in available_chunks {
                        if self.chunks_requested.contains_key(chunk_index) { continue; }
                        else if download_metadata.file_chunks[*chunk_index].downloaded { continue; }
                        else {
                            self.chunks_requested.insert(*chunk_index, Instant::now());
                            if let Some(indexes) = request_map.get_mut(peer) { indexes.push((*chunk_index, download_metadata.file_chunks[*chunk_index].position)); }
                            else { request_map.insert(*peer, vec![(*chunk_index, download_metadata.file_chunks[*chunk_index].position)]); }
                            available_chunks_requests -= 1;
                            break;
                        }
                    }
                }

                if request_map.is_empty() { return None; }
            }
            
            self.last_peers_asked_for_chunks = request_map.keys().map(|peer| peer.clone()).collect();
            return Some(request_map);
        }
        None
    }

    pub(super) fn get_file_chunks(&mut self, requested_chunks: Vec<(usize, u64)>) -> Vec<(usize, Vec<u8>)> {
        let mut file_chunks = Vec::<(usize, Vec<u8>)>::with_capacity(requested_chunks.len());
        if let Ok(mut file_reader) = File::open(&self.file_path){
            for (chunk_index, chunk_position) in requested_chunks {
                if file_reader.seek(SeekFrom::Start(chunk_position)).is_err() { break; }
                let mut raw_file_part =
                    if self.file_size < chunk_position + FILE_PIECE_SIZE as u64 { vec![0u8; (self.file_size - chunk_position) as usize] }
                    else {vec![0u8; FILE_PIECE_SIZE] };

                if file_reader.read_exact(&mut raw_file_part).is_err() { break; }
                file_chunks.push((chunk_index, raw_file_part));
            }
        }

        match self.status {
            Status::Peering | Status::Seeding => {}
            Status::Idle => { self.status = Status::Seeding; }
        }

        self.status_last_updated_time = Instant::now();
        file_chunks
    }

    pub(super) fn append_chunks_to_file(&mut self, received_chunks: Vec<(usize, Vec<u8>)>) {
        let mut chunks_appended = false;
        if let Some(download_metadata) = &mut self.download_metadata {
            let total_file_chunks_expected_count = download_metadata.file_chunks.len();
            if let Ok(mut file_writer) = OpenOptions::new().write(true).open(&self.file_path) {
                for (chunk_index, chunk_data) in received_chunks {
                    if let Some(ref mut chunk_metadata) = download_metadata.file_chunks.get_mut(chunk_index) {
                        if chunk_metadata.downloaded { continue; }
                        let mut chunk_hasher = blake3::Hasher::new();
                        chunk_hasher.update(&chunk_data);
                        if chunk_metadata.hash != *chunk_hasher.finalize().as_bytes() { continue; }
                        assert_eq!(chunk_metadata.position, chunk_index as u64 * FILE_PIECE_SIZE as u64);
                        if file_writer.seek(SeekFrom::Start(chunk_metadata.position)).is_err() { break; }
                        if file_writer.write_all(&chunk_data).is_err() { break; } 
                        assert!(file_writer.flush().is_ok());
                        download_metadata.chunks_downloaded += 1;
                        chunk_metadata.downloaded = true;
                        if !chunks_appended { chunks_appended = true; }
                        self.chunks_requested.remove(&chunk_index);
                        self.peers_with_available_chunks.values_mut().for_each(|v| { v.remove(&chunk_index); });
                        self.file_completion = {
                            let d = download_metadata.chunks_downloaded as f64 / total_file_chunks_expected_count as f64;
                            let r = d * 100.0;
                            r.floor() as u8
                        };

                        if self.file_completion >= 100 {
                            assert_eq!(download_metadata.chunks_downloaded, total_file_chunks_expected_count);
                            for chunk in &download_metadata.file_chunks {
                                assert!(chunk.downloaded == true);
                            }
                            assert!(file_writer.set_len(self.file_size).is_ok());
                            let _ = file_writer.flush();
                            drop(file_writer);
                            let old_path = self.file_path.clone();
                            self.file_path.set_file_name(self.file_name.clone());
                            assert!(std::fs::rename(old_path, &self.file_path).is_ok());
                            self.status = Status::Idle;
                            self.file_completion = 100;
                            break;
                        }
                        else if self.status != Status::Peering {
                             self.status = Status::Peering;
                        }

                        self.status_last_updated_time = Instant::now();
                    }
                }
            }
        }

        if self.file_completion >= 100 {
            self.download_metadata = None;
            self.peers_with_available_chunks.clear();
            self.chunks_requested.clear();
        }
        else if chunks_appended {
            self.flush_download_metadata();
        }
    }

    pub(super) fn get_source_peers(&self) -> HashSet<SocketAddr> {
        self.peers_with_available_chunks.keys().map(|peer| peer.clone()).collect()
    }

    pub(super) fn has_download_metadata(&self) -> bool {
        self.download_metadata.is_some()
    }

    pub(super) fn is_finished(&self) -> bool {
        self.file_completion == 100
    }

    pub(super) fn file_exists(&self) -> bool {
        self.file_path.exists()
    }

    pub(super) fn is_empty_file(&self) -> bool {
        self.file_size == 0
    }

    pub(super) fn file_size(&self) -> u64 {
        self.file_size
    }

    pub(super) fn file_name(&self) -> &str {
        &self.file_name
    }

    pub(super) fn remove_source_peer(&mut self, peer: &SocketAddr) {
        let _ = self.peers_with_available_chunks.remove(&peer);
    }

    pub(super) fn update_status_if_needed(&mut self) -> bool {
        if self.status_last_updated_time.elapsed() >= Duration::from_secs(IDLE_TIME_UNTIL_STATUS_CHANGE_SECONDS) {
            self.status = Status::Idle;
            return true;
        }
        
        false
    }
}

impl Display for SharedFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let size_to_show: String;
        let mut size = self.file_size as f64;
        let mut counter = 0;
        while size > 1024.0 && counter < 4 {
            size = size / 1024.0;
            counter += 1;
        }

        if counter == 0 { size_to_show = format!("{} B", size) }
        else if counter == 1 { size_to_show = format!("{:.0} KiB", size) }
        else if counter == 2 { size_to_show = format!("{:.0} MiB", size) }
        else { size_to_show = format!("{:.1} GiB", size) }
        
        match self.status {
            Status::Peering => write!(f, "{}: {} [{}%] Peering. Sources: {:?}", self.file_name, size_to_show, self.file_completion, self.last_peers_asked_for_chunks.iter().collect::<Vec<_>>()),
            Status::Seeding => write!(f, "{}: {} [{}%] Seeding", self.file_name, size_to_show, self.file_completion),
            Status::Idle => write!(f, "{}: {} [{}%] Idle", self.file_name, size_to_show, self.file_completion)
        }
    }
}