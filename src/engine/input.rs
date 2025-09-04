use crossterm::event::{Event, KeyCode};

use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Key {
    Esc,
    Enter,
    Up,
    Down,
    Left,
    Right
}

pub(super) fn check_input (wait_for_input: bool) -> Option<Key> {
    let mut crossterm_event = None;
    
    if wait_for_input { crossterm_event = Some(crossterm::event::read()); }
    else if let Ok(true) = crossterm::event::poll(Duration::from_secs(0)) { crossterm_event = Some(crossterm::event::read()); }

    if let Some(event) = crossterm_event {
        match event {
            Ok(Event::Key(key)) => {
                match key.code {
                    KeyCode::Enter => Some(Key::Enter),
                    KeyCode::Esc => Some(Key::Esc),
                    KeyCode::Up => Some(Key::Up),
                    KeyCode::Down => Some(Key::Down),
                    KeyCode::Left => Some(Key::Left),
                    KeyCode::Right => Some(Key::Right),
                    _ => None
                }
            }
            _ => None
        }
    }
    else { None }

}