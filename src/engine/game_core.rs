use crate::display::frame::Renderable;
use crate::engine::input::Key;
use crate::games::Game;

pub trait GameCore: Renderable {
    fn accept_key(&mut self, key: Key);
    fn is_finished(&self) -> Option<Game>;
    fn update(&mut self);
}