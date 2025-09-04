use crate::display::TerminalRenderer;
use crate::games::{Game, MainMenu, Snake, Tetris, Breakout};
use crate::engine::{input, GameCore};

use std::thread;
use std::time::Duration;

pub struct GameEngine {
    display: TerminalRenderer,
    game_running: bool
}

impl GameEngine {
    pub fn create(display: TerminalRenderer) -> Self {
        GameEngine { display: display, game_running: false }
    }

    pub fn run(&mut self) {
        let mut game: Box<dyn GameCore> = Box::new(MainMenu::create());
        loop {
            if self.game_running {
                if let Some(key) = input::check_input(false) {
                    game.accept_key(key);
                }
                
                game.update();
                self.display.draw(game.render_frame());

                if let Some(_) = game.is_finished() {
                    self.game_running = false;
                    game = Box::new(MainMenu::create());
                }
            }
            else {
                self.display.draw(game.render_frame());

                if let Some(key) = input::check_input(true) {
                    if key == crate::engine::input::Key::Esc { break; }
                    game.accept_key(key);
                }

                if let Some(game_to_play) = game.is_finished() {
                    match game_to_play {
                        Game::Snake => { game = Box::new(Snake::create()); }
                        Game::Tetris => { game = Box::new(Tetris::create()); }
                        Game::Breakout => { game = Box::new(Breakout::create()); }
                    }
                    self.game_running = true;
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}