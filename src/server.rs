use chat_app::protocol::{NetworkMessage, ClientAction, ServerAction, UserAction, AdminAction, UserName};
use chat_app::chat::Manager;

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, broadcast};
use broadcast::{Receiver, Sender};

pub struct Server {
    listener: TcpListener,
    chat_rooms_manager: Arc<Mutex<Manager>>,
    users_logged: Arc<Mutex<HashSet<UserName>>>,
    message_alert_sender: Sender<String>,
    message_alert_receiver: Receiver<String>
}

impl Server {
    pub async fn create(port: u16) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(SocketAddr::from(([127,0,0,1], port))).await?;
        let (message_alert_sender, message_alert_receiver) = broadcast::channel::<String>(10);
        Ok( Server {
            listener: listener,
            chat_rooms_manager: Arc::new(Mutex::new(Manager::create())),
            users_logged: Arc::new(Mutex::new(HashSet::new())),
            message_alert_sender: message_alert_sender,
            message_alert_receiver: message_alert_receiver
        })
    }

    pub async fn run(&mut self) -> Result<(), std::io::Error>{
        let local_address = self.listener.local_addr()?;
        println!("[{}][Server] Chat app started. Listening for connections ...", local_address);

        loop {
            match self.listener.accept().await {
                Ok((connection, remote_address)) => {
                    println!("[{}][Server] New connection from: {}", local_address, remote_address);
                    let manager = self.chat_rooms_manager.clone();
                    let users = self.users_logged.clone();
                    let message_alert_sender = self.message_alert_sender.clone();
                    let message_alert_receiver = self.message_alert_receiver.resubscribe();
                    tokio::spawn(async move {
                        Self::handle_connection(connection, remote_address, manager, users, message_alert_sender, message_alert_receiver).await;
                    });
                }
                Err(e) => {
                    println!("Error when to handle a new connection: {}", e);
                }
            }
        }
    }

    async fn handle_connection(mut connection: TcpStream, remote_address: SocketAddr, chat_manager: Arc<Mutex<Manager>>, logged_users: Arc<Mutex<HashSet<UserName>>>, message_alert_sender: Sender<String>, mut message_alert_receiver: Receiver<String>) {
        let action = chat_app::protocol::read_message(&mut connection).await;
        let username;
        let is_admin;
        match action {
            Ok(NetworkMessage::ClientMessage(ClientAction::Hello(nickname))) => {
                let mut currently_logged_users = logged_users.lock().await;
                if currently_logged_users.contains(&nickname) {
                    println!("[{}:{}] is rejected. Username is already logged. Disconnecting.", remote_address, nickname);
                    let answer = NetworkMessage::ServerMessage(ServerAction::UserRejected(format!("There is already logged user with this username: {}", nickname.clone())));
                    if let Err(e) = chat_app::protocol::send_message(&mut connection, answer).await {
                        println!("[{}:{}] Error: {} when sending a message to a client.", remote_address, nickname, e);
                    }
                    return;
                }
                else {
                    if nickname == "admin".to_string() {
                        is_admin = true;
                    }
                    else {
                        is_admin = false;
                    }

                    let answer = NetworkMessage::ServerMessage(ServerAction::UserAccepted);
                    if let Err(e) = chat_app::protocol::send_message(&mut connection, answer).await {
                        println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting.", remote_address, nickname, e);
                        return;
                    }
                    else {
                        println!("[{}:{}] new user has just logged in! Is admin: {}", remote_address, nickname, is_admin);
                        username = nickname.clone();
                        currently_logged_users.insert(nickname);
                    }
                }
            }
            Ok(_) => {
                println!("[{}] sent us other message than hello message on first message. Disconnecting", remote_address);
                return;
            }
            Err(e) => {
                println!("[{}] Disconnecting due to error when reading a message: {}", remote_address, e);
                return;
            }
        }

        let answer = NetworkMessage::ServerMessage(ServerAction::SendChatRoomsAvailable(chat_manager.lock().await.get_rooms()));
        if let Err(e) = chat_app::protocol::send_message(&mut connection, answer).await {
            println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting.", remote_address, username, e);
            return;     
        }

        loop {
            tokio::select!(
                received_message = chat_app::protocol::read_message(&mut connection) => {
                    if let Err(e) = received_message {
                        println!("[{}:{}] Error: {} when reading a message from client. Disconnecting ...", remote_address, username, e);
                        break;
                    }
                    else if let Ok(message) = received_message {
                        println!("[{}:{}] sent us a new message: {:?}", remote_address, username, message);
                        if let NetworkMessage::ClientMessage(client_message) = message {
                            match client_message {
                                ClientAction::User(action) => {
                                    match action {
                                        UserAction::FetchChatRoomMessages(room_name) => {
                                            let operation_result = chat_manager.lock().await.get_messages_from_room(room_name.clone());
                                            match operation_result {
                                                Ok(messages) => {
                                                    let answer = NetworkMessage::ServerMessage(ServerAction::SendChatRoomMessages(room_name, messages));
                                                    if let Err(e) = chat_app::protocol::send_message(&mut connection, answer).await {
                                                        println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                                        break;
                                                    }
                                                }
                                                Err(e) => {
                                                    let error_message = NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured(e));
                                                    if let Err(e) = chat_app::protocol::send_message(&mut connection, error_message).await {
                                                        println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                        UserAction::SendMessage(room_name, message) => {
                                            if let Err(e) = chat_manager.lock().await.insert_a_message_to_room(room_name.clone(), username.clone(), message) {
                                                let error_message = NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured(e));
                                                if let Err(e) = chat_app::protocol::send_message(&mut connection, error_message).await {
                                                    println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                                    break;
                                                }
                                            }
                                            else {
                                                let _ = message_alert_sender.send(room_name);
                                            }
                                        }
                                    }
                                }
                                ClientAction::Admin(action) => {
                                    if !is_admin {
                                        println!("[{}:{}] is not an admin and sent us an admin message. Disconnecting ... ", remote_address, username);
                                        let error_message = NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured("You are not an admin. You will be disconnected from chat.".to_string()));
                                                if let Err(e) = chat_app::protocol::send_message(&mut connection, error_message).await {
                                                    println!("[{}:{}] Error: {} when sending a message to a client.", remote_address, username, e);
                                                }
                                        break;
                                    }

                                    match action {
                                        AdminAction::RemoveMessage(room_name, creator_name, timestamp) => {
                                            if let Err(e) =  chat_manager.lock().await.remove_a_message_from_room(room_name.clone(), creator_name, timestamp) {
                                                let error_message = NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured(e));
                                                if let Err(e) = chat_app::protocol::send_message(&mut connection, error_message).await {
                                                    println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                                    break;
                                                }
                                            }
                                            else {
                                                let _ = message_alert_sender.send(room_name);
                                            }
                                        }
                                        AdminAction::BanUserInRoom(room_name, user_to_ban) => {
                                            if let Err(e) = chat_manager.lock().await.ban_user_in_room(room_name.clone(), user_to_ban) {
                                                let error_message = NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured(e));
                                                if let Err(e) = chat_app::protocol::send_message(&mut connection, error_message).await {
                                                    println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                                    break;
                                                }
                                            }
                                        }
                                        AdminAction::UnBanUserInRoom(room_name, user_to_unban) => {
                                            if let Err(e) = chat_manager.lock().await.unban_user_in_room(room_name.clone(), user_to_unban) {
                                                let error_message = NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured(e));
                                                if let Err(e) = chat_app::protocol::send_message(&mut connection, error_message).await {
                                                    println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                                    break;
                                                }
                                            }
                                        }
                                        AdminAction::CreateChatRoom(room_name) => {
                                            if let Err(e) = chat_manager.lock().await.add_room(room_name) {
                                                let error_message = NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured(e));
                                                if let Err(e) = chat_app::protocol::send_message(&mut connection, error_message).await {
                                                    println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                                    break;
                                                }
                                            }
                                            else {
                                                let _ = message_alert_sender.send(chat_app::chat::MAIN_ROOM_NAME.to_string());
                                            }
                                        }
                                        AdminAction::RemoveChatRoom(room_name) => {
                                            if let Err(e) = chat_manager.lock().await.delete_room(room_name) {
                                                let error_message = NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured(e));
                                                if let Err(e) = chat_app::protocol::send_message(&mut connection, error_message).await {
                                                    println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                                    break;
                                                }
                                            }
                                            else {
                                                let _ = message_alert_sender.send(chat_app::chat::MAIN_ROOM_NAME.to_string());
                                            }
                                        }
                                        AdminAction::RenameChatRoom(old_room_name, new_room_name) => {
                                            if let Err(e) = chat_manager.lock().await.rename_room(old_room_name, new_room_name) {
                                                let error_message = NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured(e));
                                                if let Err(e) = chat_app::protocol::send_message(&mut connection, error_message).await {
                                                    println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                                    break;
                                                }
                                            }
                                            else {
                                                let _ = message_alert_sender.send(chat_app::chat::MAIN_ROOM_NAME.to_string());
                                            }
                                        }
                                        AdminAction::FetchListOfBannedUsersInRoom(room_name) => {
                                            match chat_manager.lock().await.get_banned_users_in_room(room_name.clone()) {
                                                Ok(users) => {
                                                    let answer = NetworkMessage::ServerMessage(ServerAction::SendUsersBannedInRoom(room_name, users));
                                                    if let Err(e) = chat_app::protocol::send_message(&mut connection, answer).await {
                                                        println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                                        return;
                                                    }
                                                }
                                                Err(e) => {
                                                    let error_message = NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured(e));
                                                    if let Err(e) = chat_app::protocol::send_message(&mut connection, error_message).await {
                                                        println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                        AdminAction::FetchUsersOnline => {
                                            let answer = NetworkMessage::ServerMessage(ServerAction::SendUsersOnline(logged_users.lock().await.iter().cloned().collect()));
                                            if let Err(e) = chat_app::protocol::send_message(&mut connection, answer).await {
                                                println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting.", remote_address, username, e);
                                                break;
                                            }
                                        }
                                    }
                                }
                                ClientAction::Hello(_) => {
                                    println!("[{}:{}] Client sent us hello message again. Disconnecting ...", remote_address, username);
                                    break;
                                }
                            }
                        }
                        else {
                            println!("[{}:{}] Client sent us a server message. Disconnecting ...", remote_address, username);
                            break;
                        }
                    }
                }
                room_name = message_alert_receiver.recv() => {
                    if let Ok(room) = room_name {
                        if room == chat_app::chat::MAIN_ROOM_NAME {
                            let info_message = NetworkMessage::ServerMessage(ServerAction::SendChatRoomsAvailable(chat_manager.lock().await.get_rooms()));
                            if let Err(e) = chat_app::protocol::send_message(&mut connection, info_message).await {
                                println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                break;
                            }
                        }
                        else {
                            let info_message = NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(room));
                            if let Err(e) = chat_app::protocol::send_message(&mut connection, info_message).await {
                                println!("[{}:{}] Error: {} when sending a message to a client. Disconnecting ...", remote_address, username, e);
                                break;
                            }
                        }
                    }
                    else if let Err(e) = room_name {
                        println!("[{}:{}] Error: {} during reading room from internal alert. Disconnecting ...", remote_address, username, e);
                    }
                }
            );
        }

        let mut current_logged_users = logged_users.lock().await;
        current_logged_users.remove(&username);
        println!("[{}:{}] Client disconnected.", remote_address, username);
    }
}

// -----------------------------------------------
// TESTS
// -----------------------------------------------

#[tokio::test]
async fn server_tests() {
    let server_port = 8990;
    let mut server;
    {
        let new_server = Server::create(server_port).await;
        assert!(new_server.is_ok());
        server = new_server.unwrap();
    }

    tokio::spawn(async move {
        let _ = server.run().await;
    });
    
    let (mut client_bob, mut client_admin, mut client_alice);

    {
        // admin login
        let conn = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], server_port))).await;
        assert!(conn.is_ok());
        client_admin = conn.unwrap();
        
        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Hello("admin".to_string()))).await.is_ok());
        let response = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::UserAccepted));

        let chat_rooms = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(chat_rooms.is_ok());
        assert_eq!(chat_rooms.unwrap(), NetworkMessage::ServerMessage(ServerAction::SendChatRoomsAvailable(vec![chat_app::chat::MAIN_ROOM_NAME.to_string()])));

        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::FetchUsersOnline))).await.is_ok());
        let users = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(users.is_ok());
        assert_eq!(users.unwrap(), NetworkMessage::ServerMessage(ServerAction::SendUsersOnline(vec!["admin".to_string()])));
    }

    let to_delete_room = "some_trash";
    let games_room = "games_room";
    let mut memes_room = "memes_room";
    
    {
        // add room
        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::CreateChatRoom(to_delete_room.to_string())))).await.is_ok());
        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::CreateChatRoom(games_room.to_string())))).await.is_ok());
        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::CreateChatRoom(memes_room.to_string())))).await.is_ok());
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        for _ in 0..3 {
            let chat_rooms = chat_app::protocol::read_message(&mut client_admin).await;
            assert!(chat_rooms.is_ok());
            assert!(matches!(chat_rooms.unwrap(), NetworkMessage::ServerMessage(ServerAction::SendChatRoomsAvailable(_))));
        }
    }

    {
        // bob login
        let conn = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], server_port))).await;
        assert!(conn.is_ok());
        client_bob = conn.unwrap();

        assert!(chat_app::protocol::send_message(&mut client_bob, NetworkMessage::ClientMessage(ClientAction::Hello("bob".to_string()))).await.is_ok());
        let response = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::UserAccepted));

        let chat_rooms_message = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(chat_rooms_message.is_ok());
        match chat_rooms_message.unwrap() {
            NetworkMessage::ServerMessage(ServerAction::SendChatRoomsAvailable(mut chat_rooms)) => {
                chat_rooms.sort();
                assert_eq!(chat_rooms, vec![games_room.to_string(), chat_app::chat::MAIN_ROOM_NAME.to_string(), memes_room.to_string(), to_delete_room.to_string()]);
            }
            other => { panic!("Wrong type of message: {:?}", other); }
        }

        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::FetchUsersOnline))).await.is_ok());
        let users_message = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(users_message.is_ok());
        match users_message.unwrap() {
            NetworkMessage::ServerMessage(ServerAction::SendUsersOnline(mut users)) => {
                users.sort();
                assert_eq!(users, vec!["admin".to_string(), "bob".to_string()]);
            }
            other => { panic!("Wrong type of message: {:?}", other); }
        }
    }

    {
        // rename and delete room operation
        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::RemoveChatRoom(to_delete_room.to_string())))).await.is_ok());
        let old_name = memes_room;
        memes_room = "Best memes ever";
        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::RenameChatRoom(old_name.to_string(), memes_room.to_string())))).await.is_ok());
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        for _ in 0..2 {
            let chat_rooms_1 = chat_app::protocol::read_message(&mut client_admin).await;
            assert!(chat_rooms_1.is_ok());
            assert!(matches!(chat_rooms_1.unwrap(), NetworkMessage::ServerMessage(ServerAction::SendChatRoomsAvailable(_))));
            let chat_rooms_2 = chat_app::protocol::read_message(&mut client_bob).await;
            assert!(chat_rooms_2.is_ok());
            assert!(matches!(chat_rooms_2.unwrap(), NetworkMessage::ServerMessage(ServerAction::SendChatRoomsAvailable(_))));
        }
    }

    {
        // alice login
        let conn = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], server_port))).await;
        assert!(conn.is_ok());
        client_alice = conn.unwrap();

        assert!(chat_app::protocol::send_message(&mut client_alice, NetworkMessage::ClientMessage(ClientAction::Hello("alice".to_string()))).await.is_ok());
        let response = chat_app::protocol::read_message(&mut client_alice).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::UserAccepted));
        let chat_rooms_message = chat_app::protocol::read_message(&mut client_alice).await;
        assert!(chat_rooms_message.is_ok());
        match chat_rooms_message.unwrap() {
            NetworkMessage::ServerMessage(ServerAction::SendChatRoomsAvailable(mut chat_rooms)) => {
                chat_rooms.sort();
                assert_eq!(chat_rooms, vec![memes_room.to_string(), games_room.to_string(), chat_app::chat::MAIN_ROOM_NAME.to_string()]);
            }
            other => { panic!("Wrong type of message: {:?}", other); }
        }

        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::FetchUsersOnline))).await.is_ok());
        let users_message = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(users_message.is_ok());
        match users_message.unwrap() {
            NetworkMessage::ServerMessage(ServerAction::SendUsersOnline(mut users)) => {
                users.sort();
                assert_eq!(users, vec!["admin".to_string(), "alice".to_string(), "bob".to_string()]);
            }
            other => { panic!("Wrong type of message: {:?}", other); }
        }
    }

    {
        // someone tries to login as bob, who is already logged
        let tmp_conn = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], server_port))).await;
        assert!(tmp_conn.is_ok());
        let mut tmp_client = tmp_conn.unwrap();
        assert!(chat_app::protocol::send_message(&mut tmp_client, NetworkMessage::ClientMessage(ClientAction::Hello("bob".to_string()))).await.is_ok());
        let response = chat_app::protocol::read_message(&mut tmp_client).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::UserRejected("There is already logged user with this username: bob".to_string())));
    }

    {
        // John tries to be an admin
        for i in 0..8 {
            let tmp_conn = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], server_port))).await;
            assert!(tmp_conn.is_ok());
            let mut tmp_client = tmp_conn.unwrap();
            assert!(chat_app::protocol::send_message(&mut tmp_client, NetworkMessage::ClientMessage(ClientAction::Hello("John".to_string()))).await.is_ok());
            assert!(chat_app::protocol::read_message(&mut tmp_client).await.is_ok()); // accept
            assert!(chat_app::protocol::read_message(&mut tmp_client).await.is_ok()); //available rooms
            match i {
                0 => { assert!(chat_app::protocol::send_message(&mut tmp_client, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::RemoveMessage("a".to_string(), "a".to_string(), 1)))).await.is_ok()); }
                1 => { assert!(chat_app::protocol::send_message(&mut tmp_client, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::BanUserInRoom("a".to_string(), "a".to_string())))).await.is_ok()); }
                2 => { assert!(chat_app::protocol::send_message(&mut tmp_client, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::UnBanUserInRoom("a".to_string(), "a".to_string())))).await.is_ok()); }
                3 => { assert!(chat_app::protocol::send_message(&mut tmp_client, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::FetchListOfBannedUsersInRoom("a".to_string())))).await.is_ok()); }
                4 => { assert!(chat_app::protocol::send_message(&mut tmp_client, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::FetchUsersOnline))).await.is_ok()); }
                5 => { assert!(chat_app::protocol::send_message(&mut tmp_client, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::CreateChatRoom("a".to_string())))).await.is_ok()); }
                6 => { assert!(chat_app::protocol::send_message(&mut tmp_client, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::RemoveChatRoom("a".to_string())))).await.is_ok()); }
                7 => { assert!(chat_app::protocol::send_message(&mut tmp_client, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::RenameChatRoom("a".to_string(), "a".to_string())))).await.is_ok()); }
                _ => { panic!("i should be bigger than"); }
            }
            let response = chat_app::protocol::read_message(&mut tmp_client).await;

            assert!(response.is_ok());
            assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured("You are not an admin. You will be disconnected from chat.".to_string())));
        }
    }

    // sending a message to non existing room
    {
        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::User(UserAction::SendMessage("a".to_string(), "basfd".to_string())))).await.is_ok());
        let response = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured("Cannot post a message into room: there is no such room: a".to_string())));
    }

    // now good messages
    {
        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::User(UserAction::SendMessage(games_room.to_string(), "first message".to_string())))).await.is_ok());
        let mut response = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_alice).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
    }

    {
        assert!(chat_app::protocol::send_message(&mut client_bob, NetworkMessage::ClientMessage(ClientAction::User(UserAction::SendMessage(games_room.to_string(), "second message".to_string())))).await.is_ok());
        let mut response = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_alice).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
    }

    {
        assert!(chat_app::protocol::send_message(&mut client_alice, NetworkMessage::ClientMessage(ClientAction::User(UserAction::SendMessage(games_room.to_string(), "third message".to_string())))).await.is_ok());
        let mut response = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_alice).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
    }

    {
        assert!(chat_app::protocol::send_message(&mut client_bob, NetworkMessage::ClientMessage(ClientAction::User(UserAction::SendMessage(games_room.to_string(), "forth message".to_string())))).await.is_ok());
        let mut response = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_alice).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
    }

    let timestamp_to_remove;
    {
        assert!(chat_app::protocol::send_message(&mut client_bob, NetworkMessage::ClientMessage(ClientAction::User(UserAction::FetchChatRoomMessages(games_room.to_string())))).await.is_ok());
        let response = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(response.is_ok());
        match response.unwrap() {
            NetworkMessage::ServerMessage(ServerAction::SendChatRoomMessages(chat_room, chat)) => {
                assert_eq!(chat_room, games_room);
                assert_eq!(chat.len(), 4);
                assert_eq!(chat[0].get_user(), "admin");
                assert_eq!(chat[0].get_message(), "first message");
                assert_eq!(chat[1].get_user(), "bob");
                assert_eq!(chat[1].get_message(), "second message");
                assert_eq!(chat[2].get_user(), "alice");
                assert_eq!(chat[2].get_message(), "third message");
                assert_eq!(chat[3].get_user(), "bob");
                assert_eq!(chat[3].get_message(), "forth message");
                assert!(chat[0].get_timestamp() < chat[1].get_timestamp());
                assert!(chat[1].get_timestamp() < chat[2].get_timestamp());
                assert!(chat[2].get_timestamp() < chat[3].get_timestamp());

                timestamp_to_remove = chat[2].get_timestamp();
            }
            other => { panic!("Expected SendChatRoomMessages message, not {:?}", other); }
        }
    }

    //remove some mid message
    {
        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::RemoveMessage(games_room.to_string(), "alice".to_string(), timestamp_to_remove)))).await.is_ok());
        let mut response = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_alice).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(games_room.to_string())));

        assert!(chat_app::protocol::send_message(&mut client_bob, NetworkMessage::ClientMessage(ClientAction::User(UserAction::FetchChatRoomMessages(games_room.to_string())))).await.is_ok());
        let response = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(response.is_ok());
        match response.unwrap() {
            NetworkMessage::ServerMessage(ServerAction::SendChatRoomMessages(chat_room, chat)) => {
                assert_eq!(chat_room, games_room);
                assert_eq!(chat.len(), 3);
                assert_eq!(chat[0].get_user(), "admin");
                assert_eq!(chat[0].get_message(), "first message");
                assert_eq!(chat[1].get_user(), "bob");
                assert_eq!(chat[1].get_message(), "second message");
                assert_eq!(chat[2].get_user(), "bob");
                assert_eq!(chat[2].get_message(), "forth message");
                assert!(chat[0].get_timestamp() < chat[1].get_timestamp());
                assert!(chat[1].get_timestamp() < chat[2].get_timestamp());
            }
            other => { panic!("Expected SendChatRoomMessages message, not {:?}", other); }
        }
    }

    //ban and unban
    {
        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::BanUserInRoom(memes_room.to_string(), "alice".to_string())))).await.is_ok());

        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::FetchListOfBannedUsersInRoom(memes_room.to_string())))).await.is_ok());
        let mut response = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::SendUsersBannedInRoom(memes_room.to_string(), vec!["alice".to_string()])));

        assert!(chat_app::protocol::send_message(&mut client_alice, NetworkMessage::ClientMessage(ClientAction::User(UserAction::SendMessage(memes_room.to_string(), "Alice posting a message even if she is banned".to_string())))).await.is_ok());
        response = chat_app::protocol::read_message(&mut client_alice).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyErrorOccured("Cannot post a message into room: user alice is banned in: Best memes ever".to_string())));

        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::UnBanUserInRoom(memes_room.to_string(), "alice".to_string())))).await.is_ok());

        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::FetchListOfBannedUsersInRoom(memes_room.to_string())))).await.is_ok());
        response = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::SendUsersBannedInRoom(memes_room.to_string(), vec![])));

        assert!(chat_app::protocol::send_message(&mut client_alice, NetworkMessage::ClientMessage(ClientAction::User(UserAction::SendMessage(memes_room.to_string(), "Alice posting a message after she is unbanned!".to_string())))).await.is_ok());
        response = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(memes_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_alice).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(memes_room.to_string())));
        response = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::NotifyChatRoomChanged(memes_room.to_string())));
    }

    // rename existing room with messages
    {
        let new_room_name = "old good games";
        assert!(chat_app::protocol::send_message(&mut client_admin, NetworkMessage::ClientMessage(ClientAction::Admin(AdminAction::RenameChatRoom(games_room.to_string(), new_room_name.to_string())))).await.is_ok());
        let mut response = chat_app::protocol::read_message(&mut client_admin).await;
        assert!(response.is_ok());
        assert!(matches!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::SendChatRoomsAvailable(_))));
        response = chat_app::protocol::read_message(&mut client_alice).await;
        assert!(response.is_ok());
        assert!(matches!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::SendChatRoomsAvailable(_))));   
        response = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(response.is_ok());
        assert!(matches!(response.unwrap(), NetworkMessage::ServerMessage(ServerAction::SendChatRoomsAvailable(_))));
        
        assert!(chat_app::protocol::send_message(&mut client_bob, NetworkMessage::ClientMessage(ClientAction::User(UserAction::FetchChatRoomMessages(new_room_name.to_string())))).await.is_ok());
        response = chat_app::protocol::read_message(&mut client_bob).await;
        assert!(response.is_ok());
        match response.unwrap() {
            NetworkMessage::ServerMessage(ServerAction::SendChatRoomMessages(chat_room, chat)) => {
                assert_eq!(chat_room, new_room_name);
                assert_eq!(chat.len(), 3);
                assert_eq!(chat[0].get_user(), "admin");
                assert_eq!(chat[0].get_message(), "first message");
                assert_eq!(chat[1].get_user(), "bob");
                assert_eq!(chat[1].get_message(), "second message");
                assert_eq!(chat[2].get_user(), "bob");
                assert_eq!(chat[2].get_message(), "forth message");
                assert!(chat[0].get_timestamp() < chat[1].get_timestamp());
                assert!(chat[1].get_timestamp() < chat[2].get_timestamp());
            }
            other => { panic!("Expected SendChatRoomMessages message, not {:?}", other); }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
}