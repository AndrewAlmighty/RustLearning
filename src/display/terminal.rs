use crate::display::frame::Frame;

use ratatui::{
    layout::{Constraint, Layout, Flex},
    widgets::{Block, Paragraph},
    style::Style,
    text::Line,
    DefaultTerminal};

pub struct TerminalRenderer {
    terminal: DefaultTerminal
}

impl TerminalRenderer {
    pub fn create() -> Self {
        TerminalRenderer { terminal: ratatui::init() }
    }

    pub fn draw(&mut self, mut game_frame: Frame) {
        let display_width = game_frame.get_width();
        let display_height = game_frame.get_height();
        assert!(display_width <= u16::MAX as usize);
        assert!(display_height <= u16::MAX as usize);

        let _ = self.terminal.draw(move |ratatui_frame| {
            let game_area = {
                let area = ratatui_frame.area();
                let [area] = Layout::horizontal([Constraint::Length(display_width as u16 + 2)]).flex(Flex::Center).areas(area);
                let [area] = Layout::vertical([Constraint::Length(display_height as u16 + 2)]).flex(Flex::Center).areas(area);
                area
            };
            let lines_to_print: Vec<Line> = game_frame.get_display_content().chunks(game_frame.get_width() as usize).map(|row| Line::from(row.iter().collect::<String>())).collect();
            let board = Paragraph::new(lines_to_print).style(Style::default()).block(Block::bordered());
            ratatui_frame.render_widget(board, game_area);
        });
    }
}

impl Drop for TerminalRenderer {
    fn drop(&mut self) {
        ratatui::restore();
    }
}