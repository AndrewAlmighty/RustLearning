use crate::games::Game;
use crate::engine::GameCore;
use crate::engine::input::Key;
use crate::display::frame::{Renderable, Frame};

use std::time::{Instant, Duration};

use std::collections::HashSet;
use rand::rngs::ThreadRng;
use rand::Rng;

const REDUCE_INTERVAL_EVERY_TICK_BY: u64 = 1;
const BOARD_WIDTH: usize = 30;
const BOARD_HEIGHT: usize = 20;
const TETRIS_SHAPE_CHAR: char = 'O';

#[derive(PartialEq)]
#[repr(u8)]
enum Direction {
    Down,
    Left,
    Right
}

pub struct Tetris {
    board: Vec<char>,
    current_shape: HashSet<usize>,
    random_num_generator: ThreadRng,
    last_update_time: Instant,
    current_tick_interval: u64,
    is_finished: bool
}

impl Tetris {
    pub fn create() -> Self {
        let board_len = BOARD_WIDTH * BOARD_HEIGHT;
        let random_num_generator = rand::rng();
        Tetris {
            is_finished: false,
            last_update_time: Instant::now(),
            board: vec![' '; board_len],
            current_tick_interval: 500,
            random_num_generator: random_num_generator,
            current_shape: HashSet::new(),
        }
    }

    fn create_shape(&mut self) {
        
        let up_middle = BOARD_WIDTH / 2;
        match self.random_num_generator.random_range(0..7) {
            0 => { self.current_shape.extend(vec![up_middle + BOARD_WIDTH, up_middle + BOARD_WIDTH + 1, up_middle, up_middle + 1]); }
            1 => { self.current_shape.extend(vec![up_middle, up_middle + BOARD_WIDTH, up_middle + (BOARD_WIDTH * 2), up_middle + (BOARD_WIDTH * 3)]); }
            2 => { self.current_shape.extend(vec![up_middle + BOARD_WIDTH -1, up_middle + BOARD_WIDTH, up_middle, up_middle + 1]); }
            3 => { self.current_shape.extend(vec![up_middle + BOARD_WIDTH + 1, up_middle + BOARD_WIDTH, up_middle, up_middle - 1]); }
            4 => { self.current_shape.extend(vec![up_middle, up_middle + BOARD_WIDTH, up_middle + (BOARD_WIDTH * 2), up_middle + (BOARD_WIDTH *2) + 1]); }
            5 => { self.current_shape.extend(vec![up_middle, up_middle + BOARD_WIDTH, up_middle + (BOARD_WIDTH * 2), up_middle + (BOARD_WIDTH *2) - 1]); }
            6 => { self.current_shape.extend(vec![up_middle - 1, up_middle, up_middle + 1, up_middle + BOARD_WIDTH]); }
            _ => { panic!("Random should be in range <0, 6>")}
        }

        for idx in &self.current_shape {
            self.board[*idx] = TETRIS_SHAPE_CHAR;
        }
    }

    fn move_shape(&mut self, direction: Direction) {
        let mut can_move = true;
        let board_len = self.board.len();

        for idx in &self.current_shape {
            let next_position = match direction {
                Direction::Down => idx + BOARD_WIDTH,
                Direction::Left => { if *idx == 0 { return; } else { idx - 1 } }
                Direction::Right => idx + 1
            };
            if direction == Direction::Left && idx % BOARD_WIDTH == 0 { return; }
            else if direction == Direction::Right && next_position % BOARD_WIDTH == 0 { return; }
            else if next_position >= board_len || !self.current_shape.contains(&next_position) && self.board[next_position] == TETRIS_SHAPE_CHAR {
                if direction == Direction::Left || direction == Direction::Right { return; }
                can_move = false;
            }
        }
        if can_move {
            let mut moved_shape = HashSet::new();
            self.current_shape.iter().for_each(|idx| { moved_shape.insert(match direction {
                Direction::Down => *idx + BOARD_WIDTH,
                Direction::Right => *idx + 1,
                Direction::Left => *idx -1
            }); });
            moved_shape.iter().for_each(|idx| { if !self.current_shape.contains(&idx) { self.board[*idx] = TETRIS_SHAPE_CHAR; }});
            self.current_shape.iter().for_each(|idx| { if !moved_shape.contains(&idx) { self.board[*idx] = ' '; }});
            self.current_shape = moved_shape;
        }
        else { self.current_shape = HashSet::new(); }
    }

    fn remove_full_lines(&mut self) {
        let mut line_num = BOARD_HEIGHT - 1;
        let mut board_moved = false;
        while line_num > 0 {
            let begin = BOARD_WIDTH * line_num;
            let end = begin + BOARD_WIDTH;
            let mut line_full = true;
            for idx in begin..end {
                if self.board[idx] != TETRIS_SHAPE_CHAR {
                    line_full = false;
                    break;
                }
            }
            if line_full {
                board_moved = true;
                for idx in (BOARD_WIDTH..end).rev() {
                    let char_above_idx = idx - BOARD_WIDTH;
                    self.board[idx] = self.board[char_above_idx];
                }
            }
            else { line_num -= 1; }
        }
        if board_moved {
            for idx in 0..BOARD_WIDTH {
                self.board[idx] = ' ';
            }
        }
    }
}

impl Renderable for Tetris {
    fn render_frame(&self) -> Frame {
        Frame::create_frame(BOARD_WIDTH, BOARD_HEIGHT, self.board.clone())
    }
}

impl GameCore for Tetris {
    fn accept_key(&mut self, key: Key) {
        match key {
            Key::Esc => { self.is_finished = true; }
            Key::Left => { self.move_shape(Direction::Left); }   
            Key::Right => { self.move_shape(Direction::Right); }
            Key::Down => { while !self.current_shape.is_empty() { self.move_shape(Direction::Down); }  }
            _ => {}
        }
    }

    fn update(&mut self) {
        if !self.is_finished && self.last_update_time.elapsed() >= Duration::from_millis(self.current_tick_interval) {
            let mut created_shape = false;
            if self.current_shape.is_empty() {
                self.current_tick_interval = self.current_tick_interval.saturating_sub(REDUCE_INTERVAL_EVERY_TICK_BY).max(50);
                self.remove_full_lines();
                self.create_shape();
                created_shape = true;
            }

            self.move_shape(Direction::Down);
            if created_shape && self.current_shape.is_empty() { self.is_finished = true; }
            self.last_update_time = Instant::now();
        }
    }

    fn is_finished(&self) -> Option<Game> {
        if self.is_finished { Some(Game::Tetris) }
        else { None }
    }
}