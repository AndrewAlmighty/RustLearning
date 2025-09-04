use crate::games::Game;
use crate::engine::GameCore;
use crate::engine::input::Key;
use crate::display::frame::{Renderable, Frame};

use std::collections::HashSet;
use std::time::{Instant, Duration};

use rand::rngs::ThreadRng;
use rand::Rng;

const SNAKE_GAME_TICK_INTERVAL_MILLISECONDS: u64 = 150;
const BOARD_WIDTH: usize = 80;
const BOARD_HEIGHT: usize = 30;
const SNAKE_CHAR: char = '.';
const COOKIE_CHAR: char = '@';

#[repr(u8)]
enum MoveDirection {
    Up,
    Down,
    Left,
    Right
}

pub struct Snake {
    board: Vec<char>,
    snake_body: Vec<usize>,
    move_direction: MoveDirection,
    last_update_time: Instant,
    random_num_generator: ThreadRng,
    cookie_position: usize,
    is_finished: bool
}

impl Snake {
    pub fn create() -> Self {
        
        let board_len = BOARD_WIDTH * BOARD_HEIGHT;
        let mut snake_body = Vec::with_capacity(board_len);
        let snake_head = ((BOARD_HEIGHT / 2) * BOARD_WIDTH) + (BOARD_WIDTH / 2);
        snake_body.push(snake_head);
        snake_body.push(snake_head - 1);
        snake_body.push(snake_head - 2);
        let mut random_num_generator = rand::rng();
        let cookie_position = {
            let mut proposed_position = random_num_generator.random_range(0..board_len);
            while snake_body.contains(&proposed_position) {
                proposed_position = random_num_generator.random_range(0..board_len);
            }
            proposed_position
        };

        Snake {
            is_finished: false,
            last_update_time: Instant::now(),
            move_direction: MoveDirection::Right,
            snake_body: snake_body,
            board: vec![' '; board_len],
            random_num_generator: random_num_generator,
            cookie_position: cookie_position
        }
    }

    fn perform_step(&mut self) -> bool {
        let current_head_position = self.snake_body[0];
        let next_head_position;
        let mut stepped_outside_board = false;
        match self.move_direction {
            MoveDirection::Up => {
                if BOARD_WIDTH < current_head_position {
                    next_head_position = current_head_position - BOARD_WIDTH;
                }
                else { next_head_position = 0; stepped_outside_board = true; }
            }
            MoveDirection::Left => {
                if current_head_position % BOARD_WIDTH  != 0 {
                    next_head_position = current_head_position - 1;
                }
                else { next_head_position = 0; stepped_outside_board = true; }
            }
            MoveDirection::Down => {
                next_head_position = current_head_position + BOARD_WIDTH;
                if next_head_position >= self.board.len() { stepped_outside_board = true; }
            } 
            MoveDirection::Right => {
                next_head_position = current_head_position + 1;
                if next_head_position % BOARD_WIDTH == 0 { stepped_outside_board = true; }
            }
        }

        if stepped_outside_board {
            self.is_finished = true;
            false
        }
        else if next_head_position != self.cookie_position {
            self.snake_body.rotate_right(1);
            self.board[next_head_position] = SNAKE_CHAR;
            self.board[self.snake_body[0]] = ' ';
            self.snake_body[0] = next_head_position;
            false
        }
        else {
            if self.snake_body.len() == self.board.len() {
                self.is_finished = true;
                return false;
            }
            self.board[next_head_position] = SNAKE_CHAR;
            self.snake_body.push(next_head_position);
            self.snake_body.rotate_right(1);
            true
        }
    }

    fn can_perform_step(&self, direction: MoveDirection) -> bool {
        match direction {
            MoveDirection::Up | MoveDirection::Down => {
                if self.snake_body[0].abs_diff(self.snake_body[1]) > 1 {
                    return false;
                }
            }
            MoveDirection::Left | MoveDirection::Right => {
                if self.snake_body[0].abs_diff(self.snake_body[1]) == 1 {
                    return false;
                }
            }
        }
        true
    }
}

impl Renderable for Snake {
    fn render_frame(&self) -> Frame {
        Frame::create_frame(BOARD_WIDTH, BOARD_HEIGHT, self.board.clone())
    }
}

impl GameCore for Snake {
    fn accept_key(&mut self, key: Key) {
        match key {
            Key::Esc => { self.is_finished = true; }
            Key::Left => { if self.can_perform_step(MoveDirection::Left) { self.move_direction = MoveDirection::Left; }}
            Key::Right => { if self.can_perform_step(MoveDirection::Right) { self.move_direction = MoveDirection::Right; }}
            Key::Up => { if self.can_perform_step(MoveDirection::Up) { self.move_direction = MoveDirection::Up; }}
            Key::Down => { if self.can_perform_step(MoveDirection::Down) { self.move_direction = MoveDirection::Down; }}
            _ => {}
        }
    }

    fn update(&mut self) {
        if !self.is_finished && self.last_update_time.elapsed() >= Duration::from_millis(SNAKE_GAME_TICK_INTERVAL_MILLISECONDS) {
            let snake_ate_cookie = self.perform_step();
            if self.is_finished { return; }
            let board_len = self.board.len();
            let mut snake_body_indexes = HashSet::new();
            self.snake_body.iter().for_each(|idx| { if !snake_body_indexes.insert(idx) { self.is_finished = true; } } );

            if snake_ate_cookie {
                self.cookie_position = {
                    let mut proposed_position = self.random_num_generator.random_range(0..board_len);
                    while snake_body_indexes.contains(&proposed_position) {
                        proposed_position = self.random_num_generator.random_range(0..board_len);
                    }
                    proposed_position
                };
            }

            self.board[self.cookie_position] = COOKIE_CHAR;
            self.last_update_time = Instant::now();
        }
    }

    fn is_finished(&self) -> Option<Game> {
        if self.is_finished { Some(Game::Snake) }
        else { None }
    }
}