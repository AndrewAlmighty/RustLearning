use crate::reporting::display::LogSender;
use crate::reporting::log;
use crate::StatusSender;
use crate::p2p::net::message::{DISCOVER_HELLO_MESSAGE_SIZE, NetworkMessage, NodeMessage, DiscoverHello};
use crate::p2p::storage::message::StorageMessage;
use crate::p2p::net::connection::Connection;

use ipnetwork::IpNetwork;

use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Mutex;

use std::collections::{HashSet, HashMap};
use std::net::SocketAddr;
use std::sync::Arc;

const MAX_CONNECTIONS_AT_ONCE: usize = 5;

pub(super) type NodeMessageSender = tokio::sync::mpsc::UnboundedSender<NodeMessage>;
pub(super) type StorageMessageReceiver = tokio::sync::mpsc::UnboundedReceiver<StorageMessage>;
pub(super) type NetworkMessageSender = tokio::sync::mpsc::UnboundedSender<NetworkMessage>;


pub(super) struct Status {
    current_connected_peers: HashMap<SocketAddr, NetworkMessageSender>,
    peers_during_handshake: HashSet<SocketAddr>,
    potential_peers: HashSet<SocketAddr>,
    status_sender: StatusSender,
    node_address: SocketAddr
}

impl Status {
    fn create(status_sender: StatusSender, node_address: SocketAddr) -> Self {
        Status { current_connected_peers: HashMap::new(), potential_peers: HashSet::new(), status_sender: status_sender, peers_during_handshake: HashSet::new(), node_address: node_address }
    }

    pub(super) async fn send_current_status(&self) {
        assert!(self.status_sender.send(format!("Node listening address: {}\nPeers currently connected to us: {:?}", self.node_address, self.current_connected_peers.keys().collect::<Vec<_>>())).await.is_ok());
    }

    pub(super) fn get_node_address(&self) -> &SocketAddr {
        &self.node_address
    }

    fn get_potential_peers(&mut self) -> Vec<SocketAddr> {
        let mut potential_peers = Vec::new();
    
        if self.potential_peers.is_empty() {
            return potential_peers;
        }

        let mut current_connections_count = self.current_connected_peers.len();

        if current_connections_count >= MAX_CONNECTIONS_AT_ONCE {
            return potential_peers;
        }

        for peer in self.potential_peers.drain() {
            if current_connections_count >= MAX_CONNECTIONS_AT_ONCE { break; }
             else if !self.current_connected_peers.contains_key(&peer) {
                potential_peers.push(peer);
                current_connections_count += 1;
             }
        }

        potential_peers
    }

    pub(super) fn add_potential_peers(&mut self, potential_peers: Vec<SocketAddr>) {
        self.potential_peers.extend(potential_peers);
    }

    fn add_potential_peer(&mut self, potential_peer: SocketAddr) {
        self.potential_peers.insert(potential_peer);
    }

    pub(super) fn reached_connections_number_limit(&self) -> bool {
        self.current_connected_peers.len() >= MAX_CONNECTIONS_AT_ONCE
    }

    pub(super) fn peer_has_no_connections(&self) -> bool {
        self.current_connected_peers.is_empty() && self.peers_during_handshake.is_empty()
    }

    pub(super) fn is_peer_already_connected(&self, peer_address: &SocketAddr) -> bool {
        if self.current_connected_peers.contains_key(peer_address) { true }
        else { false }
    }

    pub(super) fn remove_peer_from_handshake_set(&mut self, peer: &SocketAddr) {
        let _ = self.peers_during_handshake.remove(peer);
    }

    pub(super) async fn remove_peer(&mut self, peer: &SocketAddr) {
        let _ = self.peers_during_handshake.remove(peer);
        if let Some(_) = self.current_connected_peers.remove(peer) {
            self.send_current_status().await;
        }        
    }

    pub(super) async fn remove_random_peer(&mut self) -> SocketAddr {
        assert!(self.current_connected_peers.is_empty() == false);
        let peer_to_disconnect = self.current_connected_peers.iter().next().unwrap().0.clone();
        self.remove_peer(&peer_to_disconnect).await;
        peer_to_disconnect
    }

    pub(super) fn get_current_connected_peers(&self) -> Vec<SocketAddr> {
        self.current_connected_peers.keys().map(|p| p.clone()).collect()
    }

    pub(super) fn peer_begins_handshake(&mut self, peer_address: SocketAddr) {
        self.peers_during_handshake.insert(peer_address);
    }

    pub(super) async fn peer_handshaked_successfully(&mut self, peer_address: SocketAddr, network_message_sender: NetworkMessageSender) {
        self.current_connected_peers.insert(peer_address, network_message_sender);
        self.send_current_status().await;
    }

    fn send_message_to_peer(&self, peer_address: &SocketAddr, message: NetworkMessage) -> bool {
        if let Some(msg_sender) = self.current_connected_peers.get(peer_address) {
            let _ = msg_sender.send(message);
            true
        }
        else { false }
    }
}

struct BroadcastMechanism {
    socket: UdpSocket,
    subnet: IpNetwork,
    local_broadcast_address: SocketAddr
}

pub(super) struct Node {
    log_sender: LogSender,
    p2p_listener: TcpListener,
    node_message_sender: NodeMessageSender,
    storage_message_receiver: StorageMessageReceiver,
    status: Arc<Mutex<Status>>,
    broadcast: Option<BroadcastMechanism>,
    seed_node: Option<SocketAddr>
}

impl Node {
    fn log(&self, level: log::Level, text: String) {
        assert!(self.log_sender.send(log::create("node".to_string(), level, text)).is_ok());
    }

    pub(super) async fn create(log_sender: LogSender, status_sender: StatusSender, node_message_sender: NodeMessageSender, storage_message_receiver: StorageMessageReceiver, p2p_port: u16, broadcast_subnet: Option<IpNetwork>,  broadcast_port: Option<u16>, seed_node: Option<SocketAddr>) -> Result<Node, std::io::Error> {
        let p2p_listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], p2p_port))).await?;
        let local_ip = local_ip_address::local_ip().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let broadcast_mechanism = if broadcast_subnet.is_some() && broadcast_port.is_some() {
            let broadcast_socket = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], broadcast_port.unwrap()))).await?;
            broadcast_socket.set_broadcast(true)?;            
            Some(BroadcastMechanism { socket: broadcast_socket, subnet: broadcast_subnet.unwrap(), local_broadcast_address: SocketAddr::new(local_ip, broadcast_port.unwrap()) })
        }
        else { None };

        if broadcast_mechanism.is_some() && seed_node.is_some() {
            return Err(std::io::Error::other("Cannot have both broadcast and seed_node set at once"));
        }

        if broadcast_mechanism.is_none() && seed_node.is_none() {
            return Err(std::io::Error::other("Both broadcast and seed node is not set! Must set one of them"));
        }
                
        Ok(Node {
            log_sender: log_sender,
            p2p_listener: p2p_listener,
            status: Arc::new(Mutex::new(Status::create(status_sender, SocketAddr::new(local_ip, p2p_port)))),
            broadcast: broadcast_mechanism,
            seed_node: seed_node,
            node_message_sender: node_message_sender,
            storage_message_receiver: storage_message_receiver
        })
    }

    pub(super) fn start(mut self) {
        tokio::spawn(async move {
            self.run().await;
        });
    }

    fn send_message_to_storage(&self, message: NodeMessage) {
        self.log(log::Level::Debug, format!("Sending a message to storage: {:?}", message));
        if let Err(e) = self.node_message_sender.send(message) {
            self.log(log::Level::Error, format!("Could not send a message to storage: {}", e));
        }
    }

    async fn run(&mut self) {
        {
            let p2p_addr = self.p2p_listener.local_addr();
            assert!(p2p_addr.is_ok());
            assert_ne!(self.seed_node.is_some(), self.broadcast.is_some());
    
            if let Some(seed_node) = &self.seed_node {
                self.log(log::Level::Info, format!("Node is running! P2P is listening on: {}, seed node: {}", p2p_addr.unwrap(), seed_node));
            }
            else if let Some(broadcast) = &self.broadcast {
                let broadcast_addr = broadcast.socket.local_addr();
                assert!(broadcast_addr.is_ok());
                self.log(log::Level::Info, format!("Node is running! P2P is listening on: {}, waiting for hello message on: {}, broadcast subnet: {}", p2p_addr.unwrap(), broadcast_addr.unwrap(), broadcast.subnet));
            }

            self.status.lock().await.send_current_status().await;
        }

        let (network_message_sender, mut network_message_receiver) = tokio::sync::mpsc::unbounded_channel::<NetworkMessage>();
        let mut udp_buffer = [0u8; DISCOVER_HELLO_MESSAGE_SIZE];
        let mut try_to_connect_to_p2p_interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        let mut ask_for_peers = tokio::time::interval(tokio::time::Duration::from_secs(60));
        ask_for_peers.tick().await;
        loop {
            {
                let mut status = self.status.lock().await;
                let potential_peers = status.get_potential_peers();
                let node_address = status.get_node_address();
                for peer in potential_peers {
                    if !status.is_peer_already_connected(&peer) {
                        self.connect_to(peer, node_address, &network_message_sender, false, false).await;
                    }
                }
            }

            tokio::select!(
                _ = try_to_connect_to_p2p_interval.tick() => {
                    let node_status = self.status.lock().await;
                    if node_status.peer_has_no_connections() {
                        if self.broadcast.is_some() {
                            self.broadcast_hello().await;
                        }
                        else {
                            self.connect_to(self.seed_node.unwrap(), node_status.get_node_address(), &network_message_sender, true, true).await;
                        }
                    }
                }
                _ = ask_for_peers.tick() => {
                    let node_status = self.status.lock().await;
                    if !node_status.reached_connections_number_limit() {
                        let current_connected_peers = node_status.get_current_connected_peers();
                        for peer in current_connected_peers {
                            let _ = node_status.send_message_to_peer(&peer, NetworkMessage::ListPeers(Vec::new()));
                        }
                    }
                }
                Ok((message_len, remote_address)) = async {
                    if let Some(broadcast) = &self.broadcast {
                        broadcast.socket.recv_from(&mut udp_buffer).await
                    }
                    else { 
                        Ok(std::future::pending::<(usize, std::net::SocketAddr)>().await)
                    }
                } => {
                    if self.broadcast.as_ref().unwrap().local_broadcast_address == remote_address { }
                    else if message_len != udp_buffer.len() {
                        self.log(log::Level::Debug, format!("Received an udp message from {} with length not matching hello message: {}", remote_address, message_len));
                    }
                    else {
                        match bincode::decode_from_slice::<DiscoverHello, _>(&udp_buffer, bincode::config::standard()) {
                            Ok((discover_hello, _)) => {
                                let peer = SocketAddr::new(remote_address.ip(), discover_hello.listening_port);
                                self.log(log::Level::Debug, format!("Received DiscoverHello message from: {}.", peer));
                                let node_status = self.status.lock().await;
                                if !node_status.is_peer_already_connected(&peer) {
                                    self.connect_to(peer, node_status.get_node_address(), &network_message_sender, false, true).await;
                                }
                            }
                            Err(e) => {
                                self.log(log::Level::Debug, format!("Received a message from: {} but could not decode it. Error: {}", remote_address, e));
                            }
                        }
                    }
                    udp_buffer.fill(0);
                }
                Ok((connection, remote_address)) = self.p2p_listener.accept() => {
                    self.log(log::Level::Info, format!("Incoming connection from: {}", remote_address));
                    let ask_for_peers;
                    {
                        let status = self.status.lock().await;
                        ask_for_peers = if status.peer_has_no_connections() { true } else { false };
                    }
                    self.handle_connection(connection, remote_address, network_message_sender.clone(), ask_for_peers, true).await;
                }
                message_from_connection = network_message_receiver.recv() => {
                    if message_from_connection.is_none() {
                        self.log(log::Level::Error, "Error. Received empty message from connection.".to_string());
                        continue;
                    }
                    let mut message = message_from_connection.unwrap();
                    self.log(log::Level::Debug, format!("Received {:?} from one of connections.", message));
                    match message {
                        NetworkMessage::Hello(_, _) | NetworkMessage::ConnectionAccepted(_) | NetworkMessage::ConnectionRejected(_) | NetworkMessage::ImAlive | NetworkMessage::ListPeers(_) => {
                            self.log(log::Level::Error, format!("Error. Received {:?} message from connection. It shouldn't happen.", message));
                        }
                        NetworkMessage::NewPeer(new_peer, ref peer_tried_to_connect_to, ref mut peers_informed) => {
                            let mut peers_to_send = HashSet::new();
                            let mut status = self.status.lock().await;
                            let node_address = status.get_node_address();
                            for addr in status.current_connected_peers.keys() {
                                if !peers_informed.contains(addr) {
                                    peers_to_send.insert(addr.clone());
                                    peers_informed.insert(addr.clone());
                                }
                            }

                            let amount_of_peers_to_inform = peers_to_send.len();
                            if amount_of_peers_to_inform > 0 { self.log(log::Level::Info, format!("Broadcasting message {:?} to {} peer(s)", message, amount_of_peers_to_inform)); }


                            for addr in peers_to_send {
                                if status.send_message_to_peer(&addr, message.clone()) == false {
                                    self.log(log::Level::Error, format!("Error. Sending NewPeer message to {} failed.", addr));
                                }
                            }

                            if !peer_tried_to_connect_to.contains(node_address) && !status.current_connected_peers.contains_key(&new_peer) {
                                status.add_potential_peer(new_peer);
                            }
                        }
                        NetworkMessage::ListFiles(peer, files) => {
                            if let Some(files) = files {
                                self.send_message_to_storage(NodeMessage::FilesAvailable(peer, files));
                            }
                            else {
                                self.send_message_to_storage(NodeMessage::ListFiles(peer));
                            }
                        }
                        NetworkMessage::AskForFile(file, peer) => {
                            self.send_message_to_storage(NodeMessage::AskForFile(file, peer));
                        }
                        NetworkMessage::SendMetadata(file_name, peer, file_size, metadata) => {
                            self.send_message_to_storage(NodeMessage::ReceivedMetadata(file_name, peer, file_size, metadata));
                        }
                        NetworkMessage::RequestFileChunks(peer, file_name, chunks) => {
                            self.send_message_to_storage(NodeMessage::RequestFileChunks(peer, file_name, chunks));
                        }
                        NetworkMessage::SendFileChunks(file_name, chunks) => {
                            self.send_message_to_storage(NodeMessage::ReceivedFileChunks(file_name, chunks));
                        }
                    }
                }
                message_from_storage = self.storage_message_receiver.recv() => {
                    match message_from_storage {
                        None => self.log(log::Level::Error, "Error - can't read a message from storage module.".to_string()),
                        Some(message) => {
                            match message {
                                StorageMessage::AskForFiles => {
                                    let status = self.status.lock().await;
                                    for addr in status.current_connected_peers.keys() {
                                        if status.send_message_to_peer(&addr, NetworkMessage::ListFiles(status.node_address.clone(), None)) == false {
                                            self.log(log::Level::Error, format!("Error. Sending NewPeer message to {} failed.", addr));
                                        }
                                    }
                                }
                                StorageMessage::AskPeerForFile(file_name, peer) => {
                                    let status = self.status.lock().await;
                                    if status.current_connected_peers.contains_key(&peer) {
                                        if status.send_message_to_peer(&peer, NetworkMessage::AskForFile(file_name, status.node_address.clone())) == false {
                                            self.log(log::Level::Error, format!("Error. Sending NewPeer message to {} failed.", peer));
                                        }
                                    }
                                }
                                StorageMessage::AskPeersForFileExcept(file_name, peers_from_we_already_download_file) => {
                                    let status = self.status.lock().await;
                                    for peer in status.current_connected_peers.keys() {
                                        if !peers_from_we_already_download_file.contains(&peer) {
                                            if status.send_message_to_peer(&peer, NetworkMessage::AskForFile(file_name.clone(), status.node_address.clone())) == false {
                                                self.log(log::Level::Error, format!("Error. Sending NewPeer message to {} failed.", peer));
                                            }
                                        }
                                    }
                                }
                                StorageMessage::FilesAvailable(peer, files) => {
                                    let status = self.status.lock().await;
                                    if status.current_connected_peers.contains_key(&peer) {
                                        if status.send_message_to_peer(&peer, NetworkMessage::ListFiles(status.node_address.clone(), Some(files))) == false {
                                            self.log(log::Level::Error, format!("Error. Sending NewPeer message to {} failed.", peer));
                                        }
                                    }
                                }
                                StorageMessage::SendMetadata(file_name, peer, file_size, metadata) => {
                                    let status = self.status.lock().await;
                                    if status.current_connected_peers.contains_key(&peer) {
                                        if status.send_message_to_peer(&peer, NetworkMessage::SendMetadata(file_name, status.node_address.clone(), file_size, metadata)) == false {
                                            self.log(log::Level::Error, format!("Error. Sending NewPeer message to {} failed.", peer));
                                        }
                                    }
                                }
                                StorageMessage::RequestFileChunks(peer, file_name, file_chunks) => {
                                    let status = self.status.lock().await;
                                    if status.current_connected_peers.contains_key(&peer) {
                                        if status.send_message_to_peer(&peer, NetworkMessage::RequestFileChunks(status.node_address.clone(), file_name, file_chunks)) == false {
                                            self.log(log::Level::Error, format!("Error. Sending RequestFileChunks message to {} failed.", peer));
                                        }
                                    }
                                    else {
                                        self.send_message_to_storage(NodeMessage::PeerNotConnected(peer));
                                    }
                                }
                                StorageMessage::SendFileChunks(peer, file_name, file_chunks) => {
                                    let status = self.status.lock().await;
                                    if status.current_connected_peers.contains_key(&peer) {
                                        if status.send_message_to_peer(&peer, NetworkMessage::SendFileChunks(file_name, file_chunks)) == false {
                                            self.log(log::Level::Error, format!("Error. Sending SendFileChunks message to {} failed.", peer));
                                        }
                                    }
                                    else {
                                        self.send_message_to_storage(NodeMessage::PeerNotConnected(peer));
                                    }
                                }
                            }
                        }
                    }

                }
            );
        }
    }

    async fn broadcast_hello(&self) {
        let broadcast = self.broadcast.as_ref().unwrap();
        let local_addr = self.p2p_listener.local_addr();
        assert!(local_addr.is_ok());
        let broadcast_socket_addr = broadcast.socket.local_addr();
        assert!(broadcast_socket_addr.is_ok());
        let broadcast_port = broadcast_socket_addr.unwrap().port();
        let hello_message = bincode::encode_to_vec(DiscoverHello{listening_port: local_addr.unwrap().port()}, bincode::config::standard());
        assert!(hello_message.is_ok());
        let broadcast_addr = broadcast.subnet.broadcast();
        assert!(broadcast.socket.send_to(&hello_message.unwrap(), (broadcast_addr, broadcast_port)).await.is_ok());
        self.log(log::Level::Info, format!("Broadcasted a hello message on {}:{}", broadcast_addr, broadcast_port));
    }

    async fn connect_to(&self, remote_peer: SocketAddr, node_address: &SocketAddr, network_message_sender: &NetworkMessageSender, ask_for_peers: bool, make_room_for_new_connection: bool) {
        if remote_peer == *node_address {
            return;
        }

        self.log(log::Level::Info, format!("Connection attempt to {}", remote_peer));
        match TcpStream::connect(remote_peer).await {
            Err(e) => {
                self.log(log::Level::Error, format!("Could not establish a connection with peer: {}, error: {}", remote_peer, e.to_string()));
            }

            Ok(connection) => {
                self.log(log::Level::Info, format!("Connection established to peer: {}", remote_peer));
                self.handle_connection(connection, remote_peer, network_message_sender.clone(), ask_for_peers, make_room_for_new_connection).await;
            }
        }
    }

    async fn handle_connection(&self, tcpstream: TcpStream, remote_address: SocketAddr, network_message_sender: NetworkMessageSender, ask_for_peers: bool, make_room_for_new_connection: bool) {
        let connection = Connection::create(remote_address, Arc::clone(&self.status), self.log_sender.clone());
        connection.start(tcpstream, network_message_sender, ask_for_peers, make_room_for_new_connection);
    }
}