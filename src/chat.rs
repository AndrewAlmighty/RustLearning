use crate::protocol::{ChatMessage, ChatRoomName, UserName, Timestamp};

use std::collections::{VecDeque, HashMap, HashSet};

const ROOMS_MAX_COUNT: usize = 30;
pub const MAIN_ROOM_NAME: &str = "global";

pub struct ChatRoom {
    messages: VecDeque<ChatMessage>,
    bans: HashSet<UserName>
}

pub struct Manager {
    rooms: HashMap<ChatRoomName, ChatRoom>
}

impl Manager {
    pub fn create() -> Self {
        let mut m = Manager { rooms: HashMap::with_capacity(ROOMS_MAX_COUNT) };
        assert!(m.add_room(MAIN_ROOM_NAME.to_string()).is_ok());
        m
    }

    pub fn add_room(&mut self, name: ChatRoomName) -> Result<(), String> {
        if self.rooms.len() >= ROOMS_MAX_COUNT {
            Err("Max rooms amount in chat is reached! Cannot create new ones.".to_string())
        }
        else {
            if self.rooms.contains_key(&name) {
                return Err(format!("There is already a chat room with name {}", name));
            }

            //println!("[Chat manager] New room is created: {}", name);
            self.rooms.insert(name, ChatRoom { messages: VecDeque::new(), bans: HashSet::new()});
            Ok(())
        }
    }

    pub fn rename_room(&mut self, old_name: ChatRoomName, new_name: ChatRoomName) -> Result<(), String> {
        if old_name == new_name {
            Err("Cannot rename room: old name is the same like new name".to_string())
        }
        else if old_name == MAIN_ROOM_NAME {
            Err("Cannot rename room: renaming main chat room is not possible".to_string())
        }
        else if let Some(chat_room) = self.rooms.remove(&old_name) {
            //println!("[Chat manager] Room is renamed: {} -> {}", old_name, new_name);
            self.rooms.insert(new_name, chat_room);
            Ok(())
        }
        else {
            Err(format!("Cannot rename room: there is no such room: {}", old_name))
        }
    }

    pub fn delete_room(&mut self, name: ChatRoomName) -> Result<(), String> {
        if name == MAIN_ROOM_NAME {
            Err("Cannot delete main room".to_string())
        }
        else if let Some(_) = self.rooms.remove(&name) {
            //println!("[Chat manager] Room is deleted: {}", name);
            Ok(())
        }
        else {
            Err(format!("Cannot delete room: there is no such room: {}", name))
        }
    }

    pub fn insert_a_message_to_room(&mut self, room_name: ChatRoomName, user: UserName, message: String) -> Result<(), String> {
        if let Some(chat_room) = self.rooms.get_mut(&room_name) {
            if chat_room.bans.contains(&user) {
                Err(format!("Cannot post a message into room: user {} is banned in: {}", user, room_name))
            }
            else {
                //println!("[Chat manager] new message in room: {} from: {}", room_name, user);
                chat_room.messages.push_back(ChatMessage::create(user, message));
                Ok(())
            }
        }
        else {
            Err(format!("Cannot post a message into room: there is no such room: {}", room_name))
        }
    }

    pub fn remove_a_message_from_room(&mut self, room_name: ChatRoomName, user: UserName, timestamp: Timestamp) -> Result<(), String> {
        if let Some(chat_room) = self.rooms.get_mut(&room_name) {
            if let Some(pos) = chat_room.messages.iter().position(|x| x.get_timestamp() == timestamp && x.get_user() == user) {
                //println!("[Chat manager] removing message in room: {} from {} with timestamp: {}", room_name, user, timestamp);
                let _ = chat_room.messages.remove(pos);
                return Ok(());
            }
            else {
                return Err(format!("Cannot remove a message from room: no such message in room: {}", room_name))
            }
            
        }
        else {
            Err(format!("Cannot remove a message from room: there is no such room: {}", room_name))
        }
    }

    pub fn get_messages_from_room(&self, room_name: ChatRoomName) -> Result<Vec<ChatMessage>, String> {
        if let Some(chat_room) = self.rooms.get(&room_name) {
            Ok(chat_room.messages.iter().cloned().collect())
        }
        else {
            Err(format!("Cannot get messages from room: there is no such room: {}", room_name))
        }
    }

    pub fn get_rooms(&self) -> Vec<ChatRoomName> {
        self.rooms.keys().cloned().collect()
    }  

    pub fn ban_user_in_room(&mut self, room_name: ChatRoomName, user: UserName) -> Result<(), String> {
        if let Some(chat_room) = self.rooms.get_mut(&room_name) {
            if !chat_room.bans.insert(user.clone()) {
                Err(format!("User {} is already banned in: {}", user, room_name))
            }
            else {
                //println!("[Chat manager] banning user:{} in room: {}", user, room_name);
                Ok(())
            }
        }
        else {
            Err(format!("Cannot ban user: there is no such room: {}", room_name))
        }
    }

    pub fn unban_user_in_room(&mut self, room_name: ChatRoomName, user: UserName) -> Result<(), String> {
        if let Some(chat_room) = self.rooms.get_mut(&room_name) {
            if !chat_room.bans.remove(&user) {
                Err(format!("User {} is not banned in: {}", user, room_name))
            }
            else {
                //println!("[Chat manager] unbanning user:{} in room: {}", user, room_name);
                Ok(())
            }
        }
        else {
            Err(format!("Cannot unban user: there is no such room: {}", room_name))
        }        
    }

    pub fn get_banned_users_in_room(&self, room_name: ChatRoomName) -> Result<Vec<UserName>, String> {
        if let Some(chat_room) = self.rooms.get(&room_name) {
            Ok(chat_room.bans.iter().cloned().collect())
        }
        else {
            Err(format!("Cannot get list of banned users in room: there is no such room: {}", room_name))
        }        
    }

    pub fn replace_messages_in_room(&mut self, room_name: ChatRoomName, messages: Vec<ChatMessage>) -> Result<(), String> {
        if let Some(chat_room) = self.rooms.get_mut(&room_name) {
            chat_room.messages = messages.into();
            //println!("[Chat manager] replaced messages in: {}", room_name);
            Ok(())
        }
        else {
            Err(format!("Cannot get list of banned users in room: there is no such room: {}", room_name))
        } 
    }
}


#[test]
fn test_chat_room_manager() {
    let mut manager = Manager::create();
    assert_eq!(manager.get_rooms(), vec![MAIN_ROOM_NAME]);

    {
        assert_eq!(manager.add_room(MAIN_ROOM_NAME.to_string()), Err(format!("There is already a chat room with name {}", MAIN_ROOM_NAME)));
        assert_eq!(manager.get_rooms(), vec![MAIN_ROOM_NAME]);

        let mut names = Vec::<String>::with_capacity(ROOMS_MAX_COUNT);
        for i in 0..ROOMS_MAX_COUNT-1 {
            let room_name = format!("Room no {}", i);
            assert_eq!(manager.add_room(room_name.clone()), Ok(()));
            names.push(room_name);
        }

        let mut rooms = manager.get_rooms();
        rooms.sort();
        names.sort();
        assert_eq!(rooms.len(), ROOMS_MAX_COUNT);
        for i in 0..ROOMS_MAX_COUNT-1 {
            assert_eq!(rooms[i], names[i]);
        }

        assert_eq!(rooms.last().unwrap(), MAIN_ROOM_NAME);
    }

    assert_eq!(manager.add_room("fdasfd".to_string()), Err("Max rooms amount in chat is reached! Cannot create new ones.".to_string()));

    {
        let room_to_check = "Room no 5";
        assert_eq!(manager.get_messages_from_room(room_to_check.to_string()), Ok(Vec::<ChatMessage>::new()));
        let new_name = "renamed_room";
        assert_eq!(manager.rename_room(room_to_check.to_string(), new_name.to_string()), Ok(()));
        assert_eq!(manager.rename_room(room_to_check.to_string(), new_name.to_string()), Err(format!("Cannot rename room: there is no such room: {}", room_to_check)));
        assert_eq!(manager.get_messages_from_room(new_name.to_string()), Ok(Vec::<ChatMessage>::new()));
        assert_eq!(manager.get_messages_from_room(room_to_check.to_string()), Err(format!("Cannot get messages from room: there is no such room: {}", room_to_check)));
        assert_eq!(manager.rename_room(MAIN_ROOM_NAME.to_string(), new_name.to_string()), Err("Cannot rename room: renaming main chat room is not possible".to_string()));
        assert_eq!(manager.get_rooms().len(), ROOMS_MAX_COUNT);
    }

    {
        let room_to_delete = "Room no 7";
        assert_eq!(manager.get_messages_from_room(room_to_delete.to_string()), Ok(Vec::<ChatMessage>::new()));
        assert_eq!(manager.delete_room(room_to_delete.to_string()), Ok(()));
        assert_eq!(manager.get_rooms().len(), ROOMS_MAX_COUNT - 1);
        assert_eq!(manager.get_messages_from_room(room_to_delete.to_string()), Err(format!("Cannot get messages from room: there is no such room: {}", room_to_delete)));
    }

    {
        let nonexisting_room = "fdsafvsdafsd";
        let room_to_test = "Room no 3";
        let good_user = "goodguy";
        let bad_user = "badguy";

        assert_eq!(manager.insert_a_message_to_room(nonexisting_room.to_string(), good_user.to_string(), "some innocent message".to_string()), Err(format!("Cannot post a message into room: there is no such room: {}", nonexisting_room)));
        assert_eq!(manager.insert_a_message_to_room(room_to_test.to_string(), good_user.to_string(), "some innocent message by good guy".to_string()), Ok(()));
        assert_eq!(manager.insert_a_message_to_room(room_to_test.to_string(), bad_user.to_string(), "some innocent message by bad guy".to_string()), Ok(()));
        assert_eq!(manager.get_messages_from_room(room_to_test.to_string()).unwrap().len(), 2);
        let bad_message = "some bad message by bad guy";
        assert_eq!(manager.insert_a_message_to_room(room_to_test.to_string(), bad_user.to_string(), bad_message.to_string()), Ok(()));
        assert_eq!(manager.insert_a_message_to_room(room_to_test.to_string(), good_user.to_string(), "Another innocent message".to_string()), Ok(()));
        assert_eq!(manager.get_banned_users_in_room(room_to_test.to_string()), Ok(vec![]));
        assert_eq!(manager.get_banned_users_in_room(nonexisting_room.to_string()), Err(format!("Cannot get list of banned users in room: there is no such room: {}", nonexisting_room)));
    
        {
            let messages = manager.get_messages_from_room(room_to_test.to_string()).unwrap();
            assert_eq!(messages.len(), 4);
            let timestamp = messages.iter().find(|x| x.get_user() == bad_user && x.get_message() == bad_message).unwrap().get_timestamp();
            assert_eq!(manager.remove_a_message_from_room(room_to_test.to_string(), bad_user.to_string(), timestamp), Ok(()));
        }
        assert_eq!(manager.get_messages_from_room(room_to_test.to_string()).unwrap().len(), 3);

        assert_eq!(manager.ban_user_in_room(nonexisting_room.to_string(), bad_user.to_string()), Err(format!("Cannot ban user: there is no such room: {}", nonexisting_room)));
        assert_eq!(manager.ban_user_in_room(room_to_test.to_string(), bad_user.to_string()), Ok(()));
        assert_eq!(manager.ban_user_in_room(room_to_test.to_string(), bad_user.to_string()), Err(format!("User {} is already banned in: {}", bad_user, room_to_test)));
        assert_eq!(manager.get_banned_users_in_room(room_to_test.to_string()), Ok(vec![bad_user.to_string()]));
        assert_eq!(manager.get_banned_users_in_room(room_to_test.to_string()), Ok(vec![bad_user.to_string()]));

        assert_eq!(manager.insert_a_message_to_room(room_to_test.to_string(), bad_user.to_string(), "another bad message by bad guy".to_string()), Err(format!("Cannot post a message into room: user {} is banned in: {}", bad_user, room_to_test)));
        assert_eq!(manager.insert_a_message_to_room(room_to_test.to_string(), good_user.to_string(), "another good message by good guy".to_string()), Ok(()));
        assert_eq!(manager.get_messages_from_room(room_to_test.to_string()).unwrap().len(), 4);

        assert_eq!(manager.ban_user_in_room(room_to_test.to_string(), good_user.to_string()), Ok(()));
        {
            let mut banned_list = manager.get_banned_users_in_room(room_to_test.to_string()).unwrap();
            banned_list.sort();
            assert_eq!(banned_list, vec![bad_user.to_string(), good_user.to_string()]);
        }

        assert_eq!(manager.unban_user_in_room(room_to_test.to_string(), good_user.to_string()), Ok(()));

        assert_eq!(manager.unban_user_in_room(room_to_test.to_string(), bad_user.to_string()), Ok(()));
        assert_eq!(manager.insert_a_message_to_room(room_to_test.to_string(), bad_user.to_string(), "another good message by bad guy".to_string()), Ok(()));
        assert_eq!(manager.unban_user_in_room(room_to_test.to_string(), bad_user.to_string()), Err(format!("User {} is not banned in: {}", bad_user, room_to_test)));
        assert_eq!(manager.unban_user_in_room(nonexisting_room.to_string(), bad_user.to_string()), Err(format!("Cannot unban user: there is no such room: {}", nonexisting_room)));
        assert_eq!(manager.get_banned_users_in_room(room_to_test.to_string()), Ok(vec![]));

        let messages = manager.get_messages_from_room(room_to_test.to_string()).unwrap();
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0].get_user(), good_user);
        assert_eq!(messages[1].get_user(), bad_user);
        assert_eq!(messages[2].get_user(), good_user);
        assert_eq!(messages[3].get_user(), good_user);
        assert_eq!(messages[4].get_user(), bad_user);

        let mut timestamp: Option<Timestamp> = None;

        for message in messages {
            if let Some(ts) = timestamp {
                let message_timemstamp = message.get_timestamp();
                assert!(ts < message_timemstamp);
                timestamp = Some(message_timemstamp);
            }
            else {
                timestamp = Some(message.get_timestamp());
            }
        }
    }
}