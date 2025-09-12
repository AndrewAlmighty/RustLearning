use ratatui::{
    layout::{Constraint, Layout, Flex},
    widgets::{Block, Paragraph},
    style::Style,
    text::Line,
    DefaultTerminal};


pub struct Display {
    terminal: DefaultTerminal,
    width: u16,
    height: u16
}

impl Display {
    pub fn create(width: u16, height: u16) -> Self {
        Display {width: width, height: height, terminal:  ratatui::init() }
    }

    pub fn draw(&mut self, galaxy_frame: &Vec<char>) {
        let _ = self.terminal.draw(|ratatui_frame| {
            let galaxy_area = {
                let area = ratatui_frame.area();
                let [area] = Layout::horizontal([Constraint::Length(self.width + 2)]).flex(Flex::Center).areas(area);
                let [area] = Layout::vertical([Constraint::Length(self.height + 2)]).flex(Flex::Center).areas(area);
                area
            };

            let lines_to_print: Vec<Line> = galaxy_frame.chunks(self.width as usize).map(|row| Line::from(row.iter().collect::<String>())).collect();
            let galaxy = Paragraph::new(lines_to_print).style(Style::default()).block(Block::bordered());
            ratatui_frame.render_widget(galaxy, galaxy_area);
        });

    }
}

impl Drop for Display {
    fn drop(&mut self) {
         ratatui::restore();
    }
}