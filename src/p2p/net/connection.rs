use crate::p2p::net::message::NetworkMessage;
use crate::p2p::net::node::{Status, NetworkMessageSender};

use crate::reporting::display::LogSender;
use crate::reporting::log;

use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

type NetworkMessageReceiver = tokio::sync::mpsc::UnboundedReceiver<NetworkMessage>;

pub(super) struct Connection {
    node_status: Arc<Mutex<Status>>,
    log_sender: LogSender,
    remote_peer_address: SocketAddr,
}

impl Connection {
    pub(super) fn create(remote_address: SocketAddr, node_status: Arc<Mutex<Status>>, log_sender: LogSender) -> Self {
        Connection{ node_status: node_status, log_sender: log_sender, remote_peer_address: remote_address }
    }

    fn log(&self, level: log::Level, text: String) {
        assert!(self.log_sender.send(log::create(format!("{}", self.remote_peer_address), level, text)).is_ok());
    }

    pub(super) fn start(mut self, connection: TcpStream, network_message_sender: NetworkMessageSender, ask_for_peers: bool, make_room_for_new_connection: bool) {
        tokio::spawn(async move {
            self.run(connection, network_message_sender, ask_for_peers, make_room_for_new_connection).await;
        });
    }

    async fn run(&mut self, mut connection: TcpStream, network_message_sender: NetworkMessageSender, ask_for_peers: bool, make_room_for_new_connection: bool) {
        let tmp_remote_address = self.remote_peer_address.clone();
        let node_address;
        {
            let mut node_status = self.node_status.lock().await;
            node_status.peer_begins_handshake(tmp_remote_address);
            node_address = node_status.get_node_address().clone();
        }
        match self.perform_handshake(&mut connection, node_address, &network_message_sender, ask_for_peers, make_room_for_new_connection).await {
            None => self.node_status.lock().await.remove_peer(&tmp_remote_address).await,
            Some(nmr) => {
                self.node_status.lock().await.remove_peer_from_handshake_set(&tmp_remote_address);
                let _ = tmp_remote_address;
                let _ = node_address;
                self.connection_loop(connection, network_message_sender, nmr).await;
            }
        }
    }

    async fn read_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<NetworkMessage, String> {
        let mut len_buf = [0u8; 4];
        if let Err(e) = reader.read_exact(&mut len_buf).await {
            return Err(e.to_string());
        }
        let msg_len = u32::from_be_bytes(len_buf) as usize;

        let mut msg_buf = vec![0u8; msg_len];
        if let Err(e) = reader.read_exact(&mut msg_buf).await {
            return Err(e.to_string());
        }

        match bincode::decode_from_slice(&msg_buf, bincode::config::standard()) {
            Ok((message, _)) => Ok(message),
            Err(e) => Err(e.to_string())
        }
    }

    async fn send_message<W: AsyncWriteExt + Unpin>(writer: &mut W, message: NetworkMessage) -> Result<(), String> {
        let encoded_message;
        match bincode::encode_to_vec(&message, bincode::config::standard()) {
            Ok(encoded) => encoded_message = encoded,
            Err(e) => { return Err(e.to_string()); }
        }
        if let Err(e) = writer.write_all(&(encoded_message.len() as u32).to_be_bytes()).await {
            return Err(e.to_string());
        }
        if let Err(e) = writer.write_all(&encoded_message).await {
            return Err(e.to_string());
        }   
        if let Err(e) = writer.flush().await {
            return Err(e.to_string());
        }

        Ok(())
    }

    async fn perform_handshake(&mut self, connection: &mut TcpStream, node_address: SocketAddr, network_message_sender: &NetworkMessageSender, ask_for_peers: bool, make_room_for_new_connection: bool) -> Option<NetworkMessageReceiver> {
        let mut advertise_new_peer;
        let connected_peers_at_hello;
        let mut inactivity_measuer = tokio::time::interval(tokio::time::Duration::from_secs(15));
        inactivity_measuer.tick().await;
        let network_message_receiver;
        {
            let hello_message = NetworkMessage::Hello(node_address, ask_for_peers);
            self.log(log::Level::Debug, format!("Handshake begin. Sending: {:?}", hello_message));
            if let Err(e) = Self::send_message(connection, hello_message).await {
                self.log(log::Level::Error, format!("Sending hello message failed. Error: {}", e));
                return None;
            }

            tokio::select!(
                _ = inactivity_measuer.tick() => {
                    self.log(log::Level::Error, "Waiting time for counterpart hello passed. Handshake failed".to_string());
                    return None;
                }
                message = Self::read_message(connection)  => {
                    match message {
                        Ok(NetworkMessage::Hello(peer_address, ask_for_peers)) => {
                            advertise_new_peer = ask_for_peers;
                            let communication_port = self.remote_peer_address.port();
                            self.remote_peer_address = peer_address;
                            
                            let connection_accepted;
                            let mut status = self.node_status.lock().await;
                            if status.peer_has_no_connections() { advertise_new_peer = false; }
                            connected_peers_at_hello = if ask_for_peers { status.get_current_connected_peers() }
                                                        else { Vec::new() };

                            if status.is_peer_already_connected(&self.remote_peer_address) {
                                self.log(log::Level::Info, "Peer is already connected to us. Handshake failed".to_string());
                                connection_accepted = false;
                            }
                            else if status.reached_connections_number_limit() {
                                if make_room_for_new_connection {
                                    let removed_peer = status.remove_random_peer().await;
                                    self.log(log::Level::Info, format!("Reached maximum capacity of connection. Closed connection to {} in order to make room", removed_peer));
                                    connection_accepted = true;
                                }
                                else {
                                    self.log(log::Level::Info, "Reached maximum capacity of connection. Handshake failed.".to_string());
                                    connection_accepted = false;
                                }
                            }
                            else { 
                                connection_accepted = true;
                            }

                            if connection_accepted {
                                self.log(log::Level::Debug, format!("Peer's listening port: {}. Asked for peers: {}. Connection accepted.", communication_port, ask_for_peers));
                                if let Err(e) = Self::send_message(connection, NetworkMessage::ConnectionAccepted(connected_peers_at_hello.clone())).await {
                                    self.log(log::Level::Error, format!("Sending ConnectionAccepted message failed. Error: {}", e));
                                    return None;
                                }
                                let (network_message_sender, tmp_network_message_receiver) = tokio::sync::mpsc::unbounded_channel::<NetworkMessage>();
                                status.peer_handshaked_successfully(self.remote_peer_address.clone(), network_message_sender).await;
                                network_message_receiver = tmp_network_message_receiver;
                            }
                            else {
                                self.log(log::Level::Debug, format!("Peer's listening port: {}. Asked for peers: {}. Connection rejected.", communication_port, ask_for_peers));
                                let _ = Self::send_message(connection, NetworkMessage::ConnectionRejected(connected_peers_at_hello));
                                let _ = connection.shutdown().await;
                                return None;
                            }
                        }
                        Ok(_) => {
                            self.log(log::Level::Error, format!( "Received other message than Hello. Closing connection."));
                            return None;
                        }
                        Err(e) => {
                            self.log(log::Level::Error, format!( "Error: {} when reading a message. Closing connection.", e));
                            return None;
                        }
                    }
                }
            );
        }
        inactivity_measuer.reset();

        tokio::select!(
            _ = inactivity_measuer.tick() => {
                self.log(log::Level::Error, "Waiting time for counterpart connection accepted/rejected passed. Handshake failed".to_string());
                self.node_status.lock().await.remove_peer(&self.remote_peer_address).await;
                return None;
            }
            message = Self::read_message(connection) => {
                match message {
                    Ok(NetworkMessage::ConnectionAccepted(potential_peers)) => { 
                        self.log(log::Level::Info, format!("Peer accepted connection. Potential peers to connect: {:?}", potential_peers));
                        if !potential_peers.is_empty() { self.node_status.lock().await.add_potential_peers(potential_peers); }
                    }
                    Ok(NetworkMessage::ConnectionRejected(potential_peers)) => {
                        self.log(log::Level::Info, format!("Peer rejected connection. Potential peers to connect: {:?}", potential_peers));
                        let mut node_status = self.node_status.lock().await;
                        if !potential_peers.is_empty() { node_status.add_potential_peers(potential_peers); }
                        node_status.remove_peer(&self.remote_peer_address).await;
                        return None;
                    }
                    Ok(_) => {
                        self.log(log::Level::Error, format!("Received other message than Connection Accepted/Rejected. Closing connection."));
                        return None;
                    }
                    Err(e) => {
                        self.log(log::Level::Error, format!("Error: {} when reading a message. Closing connection.", e));
                        return None;
                    }
                }
            }
        );


        if advertise_new_peer {
            let mut informed_peers = HashSet::new();
            informed_peers.insert(self.remote_peer_address.clone());
            informed_peers.insert(node_address.clone());
            let mut peer_tried_to_establish_connection_with = HashSet::new();
            peer_tried_to_establish_connection_with.extend(connected_peers_at_hello.clone());
            let message = NetworkMessage::NewPeer(self.remote_peer_address.clone(), peer_tried_to_establish_connection_with, informed_peers);
            if let Err(e) = network_message_sender.send(message) {
                self.log(log::Level::Error, format!("Sending NewPeer message after handshake to node failed. Error: {}", e));
                return None;
            }
        }

        Some(network_message_receiver)
    }

    async fn connection_loop(&mut self, mut connection: TcpStream, network_message_sender: NetworkMessageSender, mut network_message_receiver: NetworkMessageReceiver) {
        let close_connection_process = async |mut conn:TcpStream, lvl, text| {
            self.log(lvl, text);
            let mut status = self.node_status.lock().await;
            status.remove_peer(&self.remote_peer_address).await;
            let _ = conn.shutdown().await;
        };

        {
            let message = NetworkMessage::ListFiles(self.node_status.lock().await.get_node_address().clone(), None);
            self.log(log::Level::Debug, format!("Sending message {:?}", message));
            let send_result = Self::send_message(&mut connection, message).await;
            if let Err(e) = send_result {
                close_connection_process(connection, log::Level::Error, format!("Sending message failed. Error: {}", e)).await;
                return;
            }
        }

        let mut inactivity_measuer = tokio::time::interval(tokio::time::Duration::from_secs(120));
        inactivity_measuer.tick().await;
        let mut imalive_measuer = tokio::time::interval(tokio::time::Duration::from_secs(60));
        imalive_measuer.tick().await;
        loop {
            tokio::select!(
                _ = inactivity_measuer.tick() => {
                    close_connection_process(connection, log::Level::Info, "Closing connection due to inactivity".to_string()).await;
                    return;
                }
                _ = imalive_measuer.tick() => {
                    let send_result = Self::send_message(&mut connection, NetworkMessage::ImAlive).await;
                    if let Err(e) = send_result {
                        close_connection_process(connection, log::Level::Error, format!("Sending ImAlive message failed. Error: {}", e)).await;
                        return;
                    }
                }
                read_result = Self::read_message(&mut connection) => {
                    inactivity_measuer.reset();
                    match read_result {
                        Err(e) => {
                            close_connection_process(connection, log::Level::Error, format!("Error: {} when reading a message. Closing connection.", e)).await;
                            return;
                        }
                        Ok(message) => {
                            self.log(log::Level::Debug, format!("Received message {:?}", message));
                            match message {
                                NetworkMessage::Hello(_, _) | NetworkMessage::ConnectionAccepted(_) | NetworkMessage::ConnectionRejected(_) => {
                                    close_connection_process(connection, log::Level::Error, format!("Error. Received {:?} message. It shouldn't happen. Closing connection.", message)).await;
                                    return;
                                }
                                NetworkMessage::NewPeer(_, _, _) | NetworkMessage::ListFiles(_, _) | NetworkMessage::AskForFile(_, _) | NetworkMessage::SendMetadata(_, _, _, _) | NetworkMessage::RequestFileChunks(_, _, _) | NetworkMessage::SendFileChunks(_, _) => {
                                    if let Err(e) = network_message_sender.send(message) {
                                        close_connection_process(connection, log::Level::Error, format!("Sending message to node failed. Error: {}", e)).await;
                                        return;
                                    }
                                }
                                NetworkMessage::ImAlive => {}
                                NetworkMessage::ListPeers(peers) => {
                                    let mut node_status = self.node_status.lock().await;
                                    if peers.is_empty() {
                                        let send_result = Self::send_message(&mut connection, NetworkMessage::ListPeers(node_status.get_current_connected_peers())).await;
                                        if let Err(e) = send_result {
                                            close_connection_process(connection, log::Level::Error, format!("Sending ListPeers failed. Error: {}", e)).await;
                                            return;
                                        }
                                    }
                                    else { node_status.add_potential_peers(peers); }
                                }
                            }
                        }
                    }
                }
                node_message = network_message_receiver.recv() => {
                    match node_message {
                        None => {
                            self.log(log::Level::Info, "Connection message receiver is closed. Shutting down connection".to_string());
                            return;
                        }
                        Some(message) => {
                            match message {
                                NetworkMessage::Hello(_, _) | NetworkMessage::ConnectionAccepted(_) | NetworkMessage::ConnectionRejected(_) | NetworkMessage::ImAlive  => {
                                    close_connection_process(connection, log::Level::Error, format!("Error. Received {:?} message from node. It shouldn't happen. Closing connection.", message)).await;
                                    return;
                                }
                                NetworkMessage::NewPeer(_, _, _) | NetworkMessage::ListPeers(_) | NetworkMessage::ListFiles(_, _) | NetworkMessage::AskForFile(_, _) | NetworkMessage::SendMetadata(_, _, _, _) | NetworkMessage::RequestFileChunks(_, _, _) | NetworkMessage::SendFileChunks(_, _) => {
                                    imalive_measuer.reset();
                                    self.log(log::Level::Debug, format!("Sending message {:?}", message));
                                    let send_result = Self::send_message(&mut connection, message).await;
                                    if let Err(e) = send_result {
                                        close_connection_process(connection, log::Level::Error, format!("Sending message failed. Error: {}", e)).await;
                                        return;
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