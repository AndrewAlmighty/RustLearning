mod engine;
mod display;
mod games;

use crate::display::TerminalRenderer;
use crate::engine::GameEngine;

fn main() {
    let display = TerminalRenderer::create();
    let mut engine = GameEngine::create(display);
    engine.run();
}
