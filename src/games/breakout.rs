use crate::games::Game;
use crate::engine::GameCore;
use crate::engine::input::Key;
use crate::display::frame::{Renderable, Frame};

use std::time::{Instant, Duration};

const BOARD_WIDTH: usize = 57;
const BOARD_HEIGHT: usize = 30;
const BRICK_WIDTH: usize = 6;
const BRICK_CHAR: char = 'X';
const GAP_BETWEEN_BRICKS: usize = 1;
const BALL_CHAR: char = '@';
const PADDLE_SIZE: usize = 5;
const PADDLE_CHAR: char = '#';
const BREAKOUT_GAME_TICK_INTERVAL_MILLISECONDS: u64 = 150;

#[derive(Debug, PartialEq)]
#[repr(u8)]
enum BallDirection {
    DownLeft,
    DownRight,
    UpRight,
    UpLeft
}

pub struct Breakout {
    board: Vec<char>,
    last_update_time: Instant,
    paddle: Vec<usize>,
    ball_position: usize,
    ball_direction: BallDirection,
    bricks_count: u16,
    is_finished: bool
}

impl Breakout {
    pub fn create() -> Self {
        let board_len = BOARD_WIDTH * BOARD_HEIGHT;
        let mut board = vec![' '; board_len];

        let rows_with_bricks = 10;
        let bricks_per_row = (BOARD_WIDTH + GAP_BETWEEN_BRICKS) / (BRICK_WIDTH + GAP_BETWEEN_BRICKS);
        let total_width = bricks_per_row * BRICK_WIDTH + (bricks_per_row - 1) * GAP_BETWEEN_BRICKS;
        let left_margin = (BOARD_WIDTH - total_width) / 2;
        let mut bricks_count = 0;

        for row in 0..rows_with_bricks {
            for b in 0..bricks_per_row {
                let start_col = left_margin + b * (BRICK_WIDTH + GAP_BETWEEN_BRICKS);
                let base_idx = row * BOARD_WIDTH + start_col;
                for c in 0..BRICK_WIDTH {
                    board[base_idx + c] = BRICK_CHAR;
                }
                bricks_count += 1;
            }
        }

        let ball_position = rows_with_bricks * BOARD_WIDTH + (BOARD_WIDTH / 2); 
        board[ball_position] = BALL_CHAR;

        let paddle_middle_idx = board_len - (BOARD_WIDTH / 2) - 1;
        let paddle = vec![paddle_middle_idx - 2, paddle_middle_idx - 1, paddle_middle_idx, paddle_middle_idx + 1, paddle_middle_idx + 2];
        for idx in &paddle {
            board[*idx] = PADDLE_CHAR;
        }
        
        let now = Instant::now();

        Breakout {
            board: board,
            ball_direction: if now.elapsed().as_nanos() % 2 == 0 { BallDirection::DownLeft } else { BallDirection::DownRight },
            last_update_time: now,
            bricks_count: bricks_count,
            paddle: paddle,
            ball_position: ball_position,
            is_finished: false
        }
    }

    fn move_paddle(&mut self, left: bool) {
        if left && self.paddle[0] % BOARD_WIDTH != 0 {
            self.board[self.paddle[PADDLE_SIZE - 1]] = ' ';
            for idx in &mut self.paddle { *idx -= 1; }
            self.board[self.paddle[0]] = '#';
        }
        else if !left && (self.paddle[PADDLE_SIZE - 1] + 1)% BOARD_WIDTH != 0 {
            self.board[self.paddle[0]] = ' ';
            for idx in &mut self.paddle { *idx += 1; }
            self.board[self.paddle[PADDLE_SIZE - 1]] = '#';
        }
    }

    fn check_ball_collisions_with_board_edges(&mut self) {
        if self.ball_position % BOARD_WIDTH == 0 { // collision with left edge
            if self.ball_direction == BallDirection::DownLeft || self.ball_direction == BallDirection::DownRight { self.ball_direction = BallDirection::DownRight; }
            else if self.ball_direction == BallDirection::UpLeft || self.ball_direction == BallDirection::UpRight { self.ball_direction = BallDirection::UpRight; }
        }
        else if (self.ball_position + 1) % BOARD_WIDTH == 0 { // collision with right edge
            if self.ball_direction == BallDirection::DownRight || self.ball_direction == BallDirection::DownLeft { self.ball_direction = BallDirection::DownLeft; }
            else if self.ball_direction == BallDirection::UpRight || self.ball_direction == BallDirection::UpLeft { self.ball_direction = BallDirection::UpLeft; }
        }
        else if BOARD_WIDTH > self.ball_position { // collision with up edge
            if self.ball_direction == BallDirection::UpRight || self.ball_direction == BallDirection::DownRight { self.ball_direction = BallDirection::DownRight; }
            else if self.ball_direction == BallDirection::UpLeft || self.ball_direction == BallDirection::DownLeft { self.ball_direction = BallDirection::DownLeft; }
        }
        else if self.ball_position + BOARD_WIDTH > (BOARD_HEIGHT * BOARD_WIDTH) {
            self.is_finished = true;
        }
    }

    fn check_collisions_with_paddle(&mut self, next_ball_position: usize) {
        let mut ball_hit_paddle = false;
        if let Some(c) = self.board.get(next_ball_position) {
            if *c == PADDLE_CHAR { ball_hit_paddle = true; }
        }

        if ball_hit_paddle {
            match self.ball_direction {
                BallDirection::UpLeft | BallDirection::UpRight => {
                    panic!("Ball hit paddle with direction: {:?}", self.ball_direction);
                }
                BallDirection::DownLeft => {
                    if self.board[next_ball_position + 1] == ' ' { self.ball_direction = BallDirection::UpRight; }
                    else {
                        self.ball_direction = BallDirection::UpLeft;
                        self.board[self.ball_position] = ' ';
                        self.ball_position -= 1;
                        self.board[self.ball_position] = BALL_CHAR;
                    }
                }
                BallDirection::DownRight => {
                    if self.board[next_ball_position - 1] == ' ' { self.ball_direction = BallDirection::UpLeft; }
                    else {
                        self.ball_direction = BallDirection::UpRight;
                        self.board[self.ball_position] = ' ';
                        self.ball_position += 1;
                        self.board[self.ball_position] = BALL_CHAR;
                    }
                }
            }
        }
    }

    fn check_collisions_with_bricks(&mut self, next_ball_position: usize) -> bool {
        let mut ball_hit_brick = false;
        if let Some(c) = self.board.get(next_ball_position) {
            if *c == BRICK_CHAR { ball_hit_brick = true; }
        }
        else if let Some(c) = self.board.get(self.ball_position + 1) {
            if *c == BRICK_CHAR { ball_hit_brick = true; }
        }
        else if let Some(c) = self.board.get(self.ball_position - 1) {
            if *c == BRICK_CHAR { ball_hit_brick = true; }
        }
        else if let Some(c) = self.board.get(self.ball_position + BOARD_WIDTH) {
            if *c == BRICK_CHAR { ball_hit_brick = true; }
        }
        else if let Some(c) = self.board.get(self.ball_position - BOARD_WIDTH) {
            if *c == BRICK_CHAR { ball_hit_brick = true; }
        }

        if ball_hit_brick {
            {
                let mut brick_idx = next_ball_position;
                while self.board[brick_idx - 1] != ' ' {
                    brick_idx -= 1;
                }

                while self.board[brick_idx] != ' ' {
                    self.board[brick_idx] = ' ';
                    brick_idx += 1;
                }
            }

            match self.ball_direction {
                BallDirection::UpLeft => {
                    if self.board[next_ball_position + 1] == ' ' { self.ball_direction = BallDirection::DownLeft; }
                    else {
                        self.ball_direction = BallDirection::UpLeft;
                        self.board[self.ball_position] = ' ';
                        self.ball_position -= 1;
                        self.board[self.ball_position] = BALL_CHAR;
                    }
                }
                BallDirection::UpRight => {
                    if self.board[next_ball_position - 1] == ' ' { self.ball_direction = BallDirection::DownRight; }
                    else {
                        self.ball_direction = BallDirection::UpLeft;
                        self.board[self.ball_position] = ' ';
                        self.ball_position += 1;
                        self.board[self.ball_position] = BALL_CHAR;
                    }
                }
                BallDirection::DownLeft => {
                    if self.board[next_ball_position + 1] == ' ' { self.ball_direction = BallDirection::UpRight; }
                    else {
                        self.ball_direction = BallDirection::UpLeft;
                        self.board[self.ball_position] = ' ';
                        self.ball_position -= 1;
                        self.board[self.ball_position] = BALL_CHAR;
                    }
                }
                BallDirection::DownRight => {
                    if self.board[next_ball_position - 1] == ' ' { self.ball_direction = BallDirection::UpLeft; }
                    else {
                        self.ball_direction = BallDirection::UpRight;
                        self.board[self.ball_position] = ' ';
                        self.ball_position += 1;
                        self.board[self.ball_position] = BALL_CHAR;
                    }
                }
            }

            self.bricks_count -= 1;
            if self.bricks_count == 0 {
                self.is_finished = true;
            }

            return true;
        }

        return false;
    }

    fn calculate_next_ball_position(&self) -> usize {
        match self.ball_direction {
            BallDirection::DownLeft => self.ball_position + BOARD_WIDTH - 1,
            BallDirection::DownRight => self.ball_position + BOARD_WIDTH + 1,
            BallDirection::UpLeft => self.ball_position - BOARD_WIDTH - 1,
            BallDirection::UpRight => self.ball_position - BOARD_WIDTH + 1,
        }
    }

    fn move_ball (&mut self) {
        let next_ball_position = self.calculate_next_ball_position();
        self.board[next_ball_position] = BALL_CHAR;
        self.board[self.ball_position] = ' ';
        self.ball_position = next_ball_position;
    }

    fn check_collisions(&mut self) -> bool {
        self.check_ball_collisions_with_board_edges();
        if !self.is_finished {
            let next_ball_position = self.calculate_next_ball_position();
            self.check_collisions_with_paddle(next_ball_position);
            self.check_ball_collisions_with_board_edges();
            return self.check_collisions_with_bricks(next_ball_position);
        }

        false
    }
}

impl Renderable for Breakout {
    fn render_frame(&self) -> Frame {
        Frame::create_frame(BOARD_WIDTH, BOARD_HEIGHT, self.board.clone())
    }
}


impl GameCore for Breakout {
    fn accept_key(&mut self, key: Key) {
        match key {
            Key::Esc => { self.is_finished = true; }
            Key::Left => { self.move_paddle(true); }
            Key::Right => { self.move_paddle(false); }
            _ => {}
        }
    }

    fn update(&mut self) {
        if !self.is_finished && self.last_update_time.elapsed() >= Duration::from_millis(BREAKOUT_GAME_TICK_INTERVAL_MILLISECONDS) {
            if !self.check_collisions() && !self.is_finished {
                self.move_ball();
            }
            self.last_update_time = Instant::now();
        }
    }

    fn is_finished(&self) -> Option<Game> {
        if self.is_finished { Some(Game::Breakout) }
        else { None }
    }
}