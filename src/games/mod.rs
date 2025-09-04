pub mod menu;
pub mod snake;
pub mod tetris;
pub mod breakout;

pub use menu::MainMenu;
pub use snake::Snake;
pub use tetris::Tetris;
pub use breakout::Breakout;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Game {
    Breakout,
    Snake,
    Tetris
}

impl Game {
    pub(super) fn games_available_count() -> usize { 3 }
}