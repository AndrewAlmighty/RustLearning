use crossterm::event::{Event, KeyCode};

use ratatui::{
    layout::{Constraint, Layout, Position},
    style::Style,
    widgets::{Block, Paragraph},
    Frame
};

use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;

use chat_app::protocol::{ChatRoomName, UserName, ClientAction, AdminAction, UserAction, Timestamp};
use chat_app::chat::Manager;

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

enum OutputBlockTitle {
    Empty,
    AvailableRooms,
    LoggedUsers,
    BannedUsers(ChatRoomName),
    Messages(ChatRoomName)
}

pub(super) struct UserInterface {
    chat: Arc<Mutex<Manager>>,
    logged_users: Arc<Mutex<HashSet<UserName>>>,
    error_occured: Arc<Mutex<String>>,
    client_messages_queue: Sender<ClientAction>,
    current_output: String,
    current_input: String,
    current_input_index: usize,
    output_block_title: OutputBlockTitle,
    is_admin: bool,
    exit_app: bool
}

impl UserInterface {
    pub(super) fn create(is_admin: bool, chat: Arc<Mutex<Manager>>, logged_users: Arc<Mutex<HashSet<UserName>>>, client_messages_queue: Sender<ClientAction>, error_occured: Arc<Mutex<String>>) -> Self {
        UserInterface { chat: chat, logged_users: logged_users, error_occured: error_occured, client_messages_queue: client_messages_queue, current_output: String::new(), current_input: String::new(), current_input_index: 0, output_block_title: OutputBlockTitle::AvailableRooms, is_admin: is_admin, exit_app: false }
    }

    pub(super) async fn run(&mut self) {
        let mut terminal = ratatui::init();
        while !self.exit_app {
            let _ = terminal.draw(|frame| self.render(frame));

            match crossterm::event::poll(Duration::from_millis(50)) {
                    Ok(true) => {
                    let event = crossterm::event::read();
                    match event {
                        Ok(Event::Key(key)) => {
                            match key.code {
                                KeyCode::Enter => self.execute_command().await,
                                KeyCode::Char(to_insert) => self.enter_char(to_insert),
                                KeyCode::Backspace => self.delete_char(),
                                KeyCode::Left => self.move_cursor_left(),
                                KeyCode::Right => self.move_cursor_right(),
                                _ => {}
                            }
                        }
                        Err(e) => {
                            println!("Error during event reading: {}", e.to_string());
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(false) => {
                    let mut printed_error = false;
                    {
                        let mut error_occured = self.error_occured.lock().await;
                        if !error_occured.is_empty() {
                            if error_occured.as_str() == "Client disconnected" {
                                break;
                            }
                            self.output_block_title = OutputBlockTitle::Empty;
                            self.current_output = format!("Received error from server: {}", error_occured);
                            error_occured.clear();
                            printed_error = true;
                        }
                    }
                    if !printed_error {
                        match &self.output_block_title {
                            OutputBlockTitle::Empty => {}
                            OutputBlockTitle::AvailableRooms => self.list_rooms().await,
                            OutputBlockTitle::LoggedUsers => self.get_users().await,
                            OutputBlockTitle::BannedUsers(room) => self.list_bans(vec![room.clone()]).await,
                            OutputBlockTitle::Messages(room) => self.enter_room(vec![room.clone()]).await
                        }
                    }
                }
                Err(e) => {
                    println!("Error when tried to poll an event: {}", e.to_string());
                    break;
                }
            }
        }

        ratatui::restore();
    }

    fn render(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(1),
        ]);
        let [help_area, input_area, output_area] = vertical.areas(frame.area());

        let help_message = Paragraph::new("type help to print available commands or quit to quit app");
        frame.render_widget(help_message, help_area);

        let input = Paragraph::new(self.current_input.as_str()).style(Style::default()).block(Block::bordered().title("Input"));
        frame.render_widget(input, input_area);
        frame.set_cursor_position(Position::new(input_area.x + self.current_input_index as u16 + 1, input_area.y + 1));

        {
            let mut output_block = Block::bordered();

            match &self.output_block_title {
                        OutputBlockTitle::AvailableRooms => { output_block = output_block.title("Available chat rooms".to_string()); }
                        OutputBlockTitle::LoggedUsers => { output_block = output_block.title("Current logged users".to_string()); }
                        OutputBlockTitle::BannedUsers(room_name) => { output_block = output_block.title(format!("Banned users in room: {}", room_name)); }
                        OutputBlockTitle::Messages(room_name) => { output_block = output_block.title(room_name.to_string()); }
                        OutputBlockTitle::Empty => {}
            }

            let output = Paragraph::new(self.current_output.as_str()).block(output_block);
            frame.render_widget(output, output_area);
        }
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.current_input_index.saturating_sub(1);
        self.current_input_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.current_input_index.saturating_add(1);
        self.current_input_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.current_input.insert(index, new_char);
        self.move_cursor_right();
    }

    fn byte_index(&self) -> usize {
        self.current_input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.current_input_index)
            .unwrap_or(self.current_input.len())
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.current_input_index != 0;
        if is_not_cursor_leftmost {
            let current_index = self.current_input_index;
            let from_left_to_current_index = current_index - 1;
            let before_char_to_delete = self.current_input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.current_input.chars().skip(current_index);
            self.current_input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.current_input.chars().count())
    }

    fn reset_cursor(&mut self) {
        self.current_input_index = 0;
    }

    async fn execute_command(&mut self) {
        let mut cmd_and_args = self.current_input.as_str().split_whitespace().collect::<VecDeque<&str>>();

        let cmd = match cmd_and_args.pop_front() {
            Some(c) => c.to_string(),
            None => return
        };

        let mut no_args_expected = || {
            if cmd_and_args.len() > 0 {
                self.current_output = format!("{} does not expect any arguments, but provided: {:?}", cmd, cmd_and_args);
                self.output_block_title = OutputBlockTitle::Empty;
                return;
            }
        };

        match cmd.as_str() {
            "quit" => { no_args_expected(); self.exit_app = true; }
            "help" => { no_args_expected(); self.print_help(); }
            "send" => { self.send_msg( cmd_and_args.iter().map(|e| e.to_string()).collect::<Vec<String>>().join(" ")).await; }
            "list-rooms" => { no_args_expected(); self.list_rooms().await; }
            "enter" => { self.enter_room( cmd_and_args.iter().map(|e| e.to_string()).collect()).await; }
            "add-room" if self.is_admin => { self.add_room( cmd_and_args.iter().map(|e| e.to_string()).collect()).await; }
            "rename-room" if self.is_admin => { self.rename_room( cmd_and_args.iter().map(|e| e.to_string()).collect()).await; }
            "delete-room" if self.is_admin => { self.remove_room( cmd_and_args.iter().map(|e| e.to_string()).collect()).await;  }
            "get-users" if self.is_admin => { no_args_expected(); self.get_users().await; }
            "list-bans" if self.is_admin => { self.list_bans( cmd_and_args.iter().map(|e| e.to_string()).collect()).await; }
            "ban" if self.is_admin => { self.ban_user( cmd_and_args.iter().map(|e| e.to_string()).collect()).await; }
            "unban" if self.is_admin => { self.unban_user( cmd_and_args.iter().map(|e| e.to_string()).collect()).await; }
            "remove-msg" if self.is_admin => { self.remove_msg( cmd_and_args.iter().map(|e| e.to_string()).collect()).await; }
            other => {
                self.current_output = format!("{} is not a valid command", other);
                self.output_block_title = OutputBlockTitle::Empty;
            }
        }

        self.current_input.clear();
        self.reset_cursor();
    }

    fn print_help(&mut self) {
        let help;
        if self.is_admin {
            help = r##"
            send (message) - send a message to current room
            list-rooms - list available rooms
            enter (room) - move into room


            add-room (room) - add a new chat room
            rename-room (old) (new) - rename a chat room
            delete-room (room) - remove a chat room with all contents
            get-users - get list of current logged users
            list-bans (room) - get list of banned users in room
            ban (room) (user) - ban user in room
            unban (room) (user) - unban user in room
            remove-msg (room) (user) (timestamp) - remove message with timestamp in room posted by user
            "##;
        }
        else {
            help = r##"
            send (message) - send a message to current room
            list-rooms - list available rooms
            enter (room) - move into room
            "##;
        }
        self.current_output = help.lines().map(|line| line.trim_start()).collect::<Vec<&str>>().join("\n").to_string();
        self.output_block_title = OutputBlockTitle::Empty;
    }

    async fn list_rooms(&mut self) {
        let chat = self.chat.lock().await;
        self.current_output = chat.get_rooms().join("\n");
        self.output_block_title = OutputBlockTitle::AvailableRooms;
    }

    async fn enter_room(&mut self, args: Vec<String>) {
        if args.len() != 1 {
            self.output_block_title = OutputBlockTitle::Empty;
            self.current_output = format!("enter command expects only one argument - room to enter, but user provided: {:?}", args);
            return;
        }

        let room = args[0].as_str();
        let chat = self.chat.lock().await;
        match chat.get_messages_from_room(room.to_string()) {
            Ok(messages) => {
                self.current_output = messages.iter().map(|msg| format!("[{}][{}] {}", msg.get_timestamp(), msg.get_user(), msg.get_message())).collect::<Vec<String>>().join("\n");
                self.output_block_title = OutputBlockTitle::Messages(room.to_string());
            }
            Err(e) => {
                self.output_block_title = OutputBlockTitle::Empty;
                self.current_output = e;
            }
        }
    }

    async fn add_room(&mut self, args: Vec<String>) {
        if args.len() != 1 {
            self.output_block_title = OutputBlockTitle::Empty;
            self.current_output = format!("add-room command expects only one argument - room to create, but user provided: {:?}", args);
            return;
        }

        let _ = self.client_messages_queue.send(ClientAction::Admin(AdminAction::CreateChatRoom(args[0].clone()))).await;
    }

    async fn remove_room(&mut self, args: Vec<String>) {
        if args.len() != 1 {
            self.output_block_title = OutputBlockTitle::Empty;
            self.current_output = format!("remove-room command expects only one argument - room to create, but user provided: {:?}", args);
            return;
        }

        let _ = self.client_messages_queue.send(ClientAction::Admin(AdminAction::RemoveChatRoom(args[0].clone()))).await;
    }

    async fn rename_room(&mut self, args: Vec<String>) {
        if args.len() != 2 {
            self.output_block_title = OutputBlockTitle::Empty;
            self.current_output = format!("rename-room command expects  two arguments - old room name and new room name, but user provided: {:?}", args);
            return;
        }

        let _ = self.client_messages_queue.send(ClientAction::Admin(AdminAction::RenameChatRoom(args[0].clone(), args[1].clone()))).await;
    }

    async fn get_users(&mut self) {
        let _ = self.client_messages_queue.send(ClientAction::Admin(AdminAction::FetchUsersOnline)).await;
        let logged_users = self.logged_users.lock().await;
        self.current_output = logged_users.iter().map(|user| user.clone()).collect::<Vec<String>>().join("\n");
        self.output_block_title = OutputBlockTitle::LoggedUsers;
    }

    async fn list_bans(&mut self, args: Vec<String>) {
        if args.len() != 1 {
            self.output_block_title = OutputBlockTitle::Empty;
            self.current_output = format!("list-bans command expects only one argument - room from which banned users list will be fetch, but user provided: {:?}", args);
            return;
        }

        let room = args[0].as_str();
        let _ = self.client_messages_queue.send(ClientAction::Admin(AdminAction::FetchListOfBannedUsersInRoom(room.to_string()))).await;
        let chat = self.chat.lock().await;

        match chat.get_banned_users_in_room(room.to_string()) {
            Ok(bans) => {
                self.current_output = bans.iter().map(|user| user.clone()).collect::<Vec<String>>().join("\n");
                self.output_block_title = OutputBlockTitle::BannedUsers(room.to_string());
            }
            Err(e) => {
                self.current_output = e;
                self.output_block_title = OutputBlockTitle::Empty;
            }
        }
        
    }

    async fn ban_user(&mut self, args: Vec<String>) {
        if args.len() != 2 {
            self.output_block_title = OutputBlockTitle::Empty;
            self.current_output = format!("ban command expects two argument - room where user will be banned and username, but user provided: {:?}", args);
            return;
        }

        let _ = self.client_messages_queue.send(ClientAction::Admin(AdminAction::BanUserInRoom(args[0].clone(), args[1].clone()))).await;
    }

    async fn unban_user(&mut self, args: Vec<String>) {
        if args.len() != 2 {
            self.output_block_title = OutputBlockTitle::Empty;
            self.current_output = format!("unban command expects two argument - room where user will be unbanned and username, but user provided: {:?}", args);
            return;
        }

        let _ = self.client_messages_queue.send(ClientAction::Admin(AdminAction::UnBanUserInRoom(args[0].clone(), args[1].clone()))).await;
    }

    async fn send_msg(&mut self, message: String) {
        if message.is_empty() {
            self.output_block_title = OutputBlockTitle::Empty;
            self.current_output = "send command expects only one argument - a message, but use".to_string();
            return;
        }

        match &self.output_block_title {
            OutputBlockTitle::Messages(room_name) => {
                let _ = self.client_messages_queue.send(ClientAction::User(UserAction::SendMessage(room_name.to_string(), message))).await;
            }
            _ => { 
                self.output_block_title = OutputBlockTitle::Empty;
                self.current_output = "You need to enter any chat rooms to be able to send a message".to_string();
            }
        }
    }

    async fn remove_msg(&mut self, args: Vec<String>) {
        if args.len() != 3 {
            self.output_block_title = OutputBlockTitle::Empty;
            self.current_output = format!("remove-msg command expects three argument - room where is messege to remove, username and timestamp of message, but user provided: {:?}", args);
            return;
        }

        match args[2].parse::<Timestamp>() {
            Ok(timestamp) => { let _ = self.client_messages_queue.send(ClientAction::Admin(AdminAction::RemoveMessage(args[0].clone(), args[1].clone(), timestamp))).await; }
            Err(e) => {
                self.output_block_title = OutputBlockTitle::Empty;
                self.current_output = format!("remove-msg: timestamp args: {} is not proper value - err: {}", args[2], e.to_string());
            }
         }
        
    }
}
