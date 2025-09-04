pub struct Frame {
    display_width: usize,
    display_height: usize,
    display_contents: Vec<char>
}

impl Frame {
    pub fn create_frame(width: usize, height: usize, display_contents: Vec<char>) -> Self {
        Frame {display_width: width, display_height: height, display_contents: display_contents}
    }

    pub fn get_width(&self) -> usize {
        self.display_width
    }

    pub fn get_height(&self) -> usize {
        self.display_height
    }

    pub fn get_display_content(&mut self) -> Vec<char> {
        std::mem::take(&mut self.display_contents)
    }
}

pub trait Renderable {
    fn render_frame(&self) -> Frame;
}