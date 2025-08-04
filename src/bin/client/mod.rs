mod ui;

use ui::UserInterface;

use chat_app::protocol::{NetworkMessage, ServerAction, UserAction, ClientAction, UserName, ChatRoomName};
use chat_app::chat::Manager;

use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, mpsc::Receiver};

use std::collections::HashSet;
use std::sync::Arc;

pub struct Client {
    ui: ui::UserInterface,
    connection: TcpStream,
    username: UserName,
    chat: Arc<Mutex<Manager>>,
    logged_users: Arc<Mutex<HashSet<UserName>>>,
    client_messages_queue: Receiver<ClientAction>,
    error_occured: Arc<Mutex<String>>
}

impl Client {
    pub async fn create(server_address: String, username: String) -> Result<Client, String> {
        match TcpStream::connect(server_address).await {
            Err(e) => Err(e.to_string()),
            Ok(mut conn) => {
                if let Err(e) = chat_app::protocol::send_message(&mut conn, NetworkMessage::ClientMessage(ClientAction::Hello(username.clone()))).await {
                    return Err(e.to_string());
                }

                match chat_app::protocol::read_message(&mut conn).await {
                    Err(e) =>  Err(e.to_string()),
                    Ok(response) => {
                        match response {
                            NetworkMessage::ServerMessage(ServerAction::UserAccepted) => {
                                let chat = Arc::new(Mutex::new(Manager::create()));
                                let logged_users = Arc::new(Mutex::new(HashSet::new()));
                                let (sender, receiver) = mpsc::channel(5);
                                let error_occured = Arc::new(Mutex::new(String::new()));
                                Ok( Client{ 
                                connection: conn,
                                chat: Arc::clone(&chat),
                                logged_users: Arc::clone(&logged_users),
                                ui: UserInterface::create(if username.as_str() == "admin" { true } else { false }, Arc::clone(&chat), Arc::clone(&logged_users), sender, Arc::clone(&error_occured)),
                                username: username,
                                client_messages_queue: receiver,
                                error_occured: Arc::clone(&error_occured)
                            })},
                            NetworkMessage::ServerMessage(ServerAction::UserRejected(reason)) => Err(reason),
                            other => Err(format!("Received message: {:?} which is not expected", other))
                        }
                    }
                }
            }
        }
    }

    pub async fn run(mut self) {
        println!("[{}] Client connected to server!", self.username);
        {
            let connection = self.connection;
            let logged_users = Arc::clone(&self.logged_users);
            let chat = Arc::clone(&self.chat);
            let client_messages_queue = self.client_messages_queue;
            let username = self.username;
            let error_occured = self.error_occured;
            tokio::spawn(async move {
                Self::connection_thread(connection, chat, logged_users, client_messages_queue, username, error_occured).await;
            });
        }

        self.ui.run().await;
    }

    async fn connection_thread(mut connection: TcpStream, chat: Arc<Mutex<Manager>>, logged_users: Arc<Mutex<HashSet<UserName>>>, mut client_messages_queue: Receiver<ClientAction>, username: UserName, error_occured: Arc<Mutex<String>>) {
        loop {
            tokio::select!(
                received_message = chat_app::protocol::read_message(&mut connection) => {
                    match received_message {
                        Ok(NetworkMessage::ServerMessage(action)) => {
                            match action {
                                ServerAction::UserAccepted | ServerAction::UserRejected(_)=> {
                                     println!("[{} Error: we do not expect answer for hello message now from server: {:?}.", username, connection.peer_addr());
                                }
                                ServerAction::SendUsersOnline(users) => {
                                    let mut logged_users = logged_users.lock().await;
                                    logged_users.clear();
                                    for user in users {
                                        logged_users.insert(user);
                                    }
                                }
                                ServerAction::SendChatRoomsAvailable(rooms) => {
                                    let mut rooms_on_server = rooms.iter().map(|room| room.clone()).collect::<HashSet<String>>();
                                    let mut diff = Vec::<ChatRoomName>::new();
                                    let mut chat = chat.lock().await;
                                    let current_rooms = chat.get_rooms();

                                    for room in current_rooms {
                                        if rooms_on_server.contains(&room) {
                                            rooms_on_server.remove(&room);
                                        }
                                        else {
                                            diff.push(room);
                                        }
                                    }

                                    if !diff.is_empty() {
                                        for room in diff {
                                            let _ = chat.delete_room(room);
                                        }
                                    }

                                    if !rooms_on_server.is_empty() {
                                        for room in rooms_on_server {
                                            let _ = chat.add_room(room.clone());
                                            if let Err(e) = chat_app::protocol::send_message(&mut connection, NetworkMessage::ClientMessage(ClientAction::User(UserAction::FetchChatRoomMessages(room)))).await {
                                                println!("[{} Error: {} when sending a message to a server: {:?}.", username, e, connection.peer_addr());
                                                break;
                                            }
                                        }
                                    }
                                    if let Err(e) = chat_app::protocol::send_message(&mut connection, NetworkMessage::ClientMessage(ClientAction::User(UserAction::FetchChatRoomMessages(chat_app::chat::MAIN_ROOM_NAME.to_string())))).await {
                                        println!("[{} Error: {} when sending a message to a server: {:?}.", username, e, connection.peer_addr());
                                        break;
                                    }
                                }
                                ServerAction::SendUsersBannedInRoom(room, users) => {
                                    let mut banned_users_on_server = users.iter().map(|user| user.clone()).collect::<HashSet<String>>();
                                    let mut diff = Vec::<UserName>::new();
                                    let mut chat = chat.lock().await;
                                    let current_bans = chat.get_banned_users_in_room(room.clone());

                                    if current_bans.is_ok() {
                                        for ban in current_bans.unwrap() {
                                            if banned_users_on_server.contains(&ban) {
                                                banned_users_on_server.remove(&room);
                                            }
                                            else {
                                                diff.push(ban);
                                            }
                                        }

                                        if !diff.is_empty() {
                                            for ban in diff {
                                                let _ = chat.unban_user_in_room(room.clone(), ban);
                                            }
                                        }

                                        if !banned_users_on_server.is_empty() {
                                            for ban in banned_users_on_server {
                                                let _ = chat.ban_user_in_room(room.clone(), ban);
                                            }
                                        }
                                    }
                                }
                                ServerAction::SendChatRoomMessages(room, messages) => {
                                    let mut chat = chat.lock().await;
                                    let _ = chat.replace_messages_in_room(room, messages);
                                }
                                ServerAction::NotifyChatRoomChanged(room) => {
                                    if let Err(e) = chat_app::protocol::send_message(&mut connection, NetworkMessage::ClientMessage(ClientAction::User(UserAction::FetchChatRoomMessages(room)))).await {
                                        println!("[{} Error: {} when sending a message to a server: {:?}.", username, e, connection.peer_addr());
                                        break;
                                    }
                                }
                                ServerAction::NotifyErrorOccured(e) => {
                                    let mut error_occured = error_occured.lock().await;
                                    *error_occured = e.to_string();
                                }
                            }
                        }
                        Err(e) => {
                            println!("[{} Error: {} when reading a message from a server: {:?}.", username, e, connection.peer_addr());
                            break;
                        }
                        _ => {}
                    }
                }
                message_to_send = client_messages_queue.recv() => {
                    if let Err(e) = chat_app::protocol::send_message(&mut connection, NetworkMessage::ClientMessage(message_to_send.unwrap())).await {
                        println!("[{} Error: {} when sending a message to a server: {:?}.", username, e, connection.peer_addr());
                        break;
                    }
                }
            );
        }

        let mut error_occured = error_occured.lock().await;
        *error_occured = "Client disconnected".to_string();
        println!("[{}] Connection thread stopped - client disconnected", username);
    }
}