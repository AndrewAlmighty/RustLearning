use bincode::{Encode, Decode};

use chrono::Utc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub type Timestamp = i64;
pub type UserName = String;
pub type ChatRoomName = String;

#[derive(Encode, Decode, Clone, Debug, PartialEq)]
pub struct ChatMessage {
    timestamp: Timestamp,
    user: UserName,
    message: String
}

impl ChatMessage {
    pub fn create(user: UserName, message: String) -> Self {
        ChatMessage {
            timestamp: Utc::now().timestamp_micros(),
            user: user,
            message: message
        }
    }

    pub fn get_timestamp(&self) -> Timestamp {
        self.timestamp
    }

    pub fn get_user(&self) -> &str {
        &self.user
    }

    pub fn get_message(&self) -> &str {
        &self.message
    }
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub enum NetworkMessage {
    ClientMessage(ClientAction),
    ServerMessage(ServerAction)
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub enum ClientAction {
    Hello(UserName),
    User(UserAction),
    Admin(AdminAction)
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub enum UserAction {
    FetchChatRoomMessages(ChatRoomName),
    SendMessage(ChatRoomName, String)
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub enum AdminAction {
    RemoveMessage(ChatRoomName, UserName, Timestamp),
    BanUserInRoom(ChatRoomName, UserName),
    UnBanUserInRoom(ChatRoomName, UserName),
    FetchListOfBannedUsersInRoom(ChatRoomName),
    FetchUsersOnline,
    CreateChatRoom(ChatRoomName),
    RemoveChatRoom(ChatRoomName),
    RenameChatRoom(ChatRoomName, ChatRoomName),
}

#[derive(Encode, Decode, Debug, PartialEq)]
pub enum ServerAction {
    UserAccepted,
    UserRejected(String),
    SendChatRoomMessages(ChatRoomName, Vec<ChatMessage>), //
    SendUsersOnline(Vec<UserName>),//
    SendChatRoomsAvailable(Vec<ChatRoomName>),//
    SendUsersBannedInRoom(ChatRoomName, Vec<UserName>),//
    NotifyChatRoomChanged(ChatRoomName),
    NotifyErrorOccured(String)
}

pub async fn read_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<NetworkMessage, String> {
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
        Ok((action, _)) => Ok(action),
        Err(e) => Err(e.to_string())
    }
}

pub async fn send_message<W: AsyncWriteExt + Unpin>(writer: &mut W, message: NetworkMessage) -> Result<(), String> {
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

#[test]
fn protocol_encode_decode_test() {
    use std::io::Cursor;
    use bincode::{encode_into_std_write, decode_from_std_read};
    {
        let action = ClientAction::Hello("george".to_string());
        let mut buffer = Vec::new();
        encode_into_std_write(&action, &mut buffer, bincode::config::standard()).unwrap();
        let mut cursor = Cursor::new(buffer);
        let decoded_action: ClientAction = decode_from_std_read(&mut cursor, bincode::config::standard()).unwrap();
        assert_eq!(action, decoded_action);
    }

    {
        let action = ClientAction::User(UserAction::SendMessage("Room no 1".to_string(), "I have a lot to tell you. I was on a trip ... blablablablrfsa\nfdsr".to_string()));
        let mut buffer = Vec::new();
        encode_into_std_write(&action, &mut buffer, bincode::config::standard()).unwrap();
        let mut cursor = Cursor::new(buffer);
        let decoded_action: ClientAction = decode_from_std_read(&mut cursor, bincode::config::standard()).unwrap();
        assert_eq!(action, decoded_action);
    }

    {
        let action = ClientAction::Admin(AdminAction::RemoveMessage("Room no 1".to_string(), "George".to_string(), 143253244324));
        let mut buffer = Vec::new();
        encode_into_std_write(&action, &mut buffer, bincode::config::standard()).unwrap();
        let mut cursor = Cursor::new(buffer);
        let decoded_action: ClientAction = decode_from_std_read(&mut cursor, bincode::config::standard()).unwrap();
        assert_eq!(action, decoded_action);
    }

    {
        let action = ServerAction::SendChatRoomMessages("Room no 1".to_string(), vec![
            ChatMessage{timestamp: 3241, user: "George".to_string(), message: "First message".to_string()},
            ChatMessage{timestamp: 1234, user: "Michael".to_string(), message: "Second message".to_string()},
            ChatMessage{timestamp: 532415, user: "John".to_string(), message: "Third message".to_string()},
            ChatMessage{timestamp: 65623, user: "Paul".to_string(), message: "Forth message".to_string()}
            ]);

        let mut buffer = Vec::new();
        encode_into_std_write(&action, &mut buffer, bincode::config::standard()).unwrap();
        let mut cursor = Cursor::new(buffer);
        let decoded_action: ServerAction = decode_from_std_read(&mut cursor, bincode::config::standard()).unwrap();
        assert_eq!(action, decoded_action);
    }
}