use crate::games::Game;
use crate::engine::GameCore;
use crate::engine::input::Key;
use crate::display::frame::{Renderable, Frame};

use std::collections::BTreeSet;

pub struct MainMenu {
    games_available: BTreeSet<Game>,
    current_chosen_game: usize,
    game_chosen: bool
}

impl MainMenu {
    pub fn create() -> Self {
        let mut games_available = BTreeSet::<Game>::new();
        games_available.insert(Game::Snake);
        games_available.insert(Game::Breakout);
        games_available.insert(Game::Tetris);
        assert_eq!(games_available.len(), Game::games_available_count());

        MainMenu {
            games_available: games_available,
            current_chosen_game: 0,
            game_chosen: false
        }
    }
}

impl Renderable for MainMenu {
    fn render_frame(&self) -> Frame {
        let games_available_count = self.games_available.len();
        let mut display_width = 0usize;

        let mut available_games = Vec::<String>::with_capacity(games_available_count);
        for game in &self.games_available {
            let is_current_chosen = *game == *self.games_available.iter().nth(self.current_chosen_game).unwrap();
            let game_as_str = match game {
                Game::Breakout => "Breakout",
                Game::Snake => "Snake",
                Game::Tetris => "Tetris"
            };

            let game_name_len = game_as_str.len();
            if game_name_len > display_width {
                display_width = game_name_len + 10;
            }

            let option = 
                if is_current_chosen { format!(" -> {}", game_as_str) }
                else { format!("    {}", game_as_str) };

            available_games.push(option);
        }

        let display_height = games_available_count + 2;
        let mut board_to_display = Vec::<char>::with_capacity(display_width * display_height);
        board_to_display.extend(std::iter::repeat(' ').take(display_width));
        for game in available_games {
            let whitespaces_to_fill = display_width - game.len();
            board_to_display.extend(game.chars());
            board_to_display.extend(std::iter::repeat(' ').take(whitespaces_to_fill));
        }
        board_to_display.extend(std::iter::repeat(' ').take(display_width));
        Frame::create_frame(display_width, display_height, board_to_display)
    }
}

impl GameCore for MainMenu {
    fn accept_key(&mut self, key: Key) {
        match key {
            Key::Up => { if self.current_chosen_game > 0 { self.current_chosen_game -= 1; }}
            Key::Down => { if self.current_chosen_game < self.games_available.len() - 1 { self.current_chosen_game += 1; }}
            Key::Enter => { self.game_chosen = true; }
            _ => { }
        }
    }

    fn update(&mut self) {}

    fn is_finished(&self) -> Option<Game> {
        if self.game_chosen { Some(*self.games_available.iter().nth(self.current_chosen_game).unwrap()) }
        else { None }
    }
}