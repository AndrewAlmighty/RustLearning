mod display;
mod simulator;

use crossterm::{event, event::Event};

use std::thread;
use std::time::Duration;

#[derive(clap::Parser)]
#[command(
    name = "galaxy-simulator",
    about = "ASCII galaxy simulation",
    after_help = "To stop simulation, just press any key"
)]
pub struct Config {
    #[arg(long, help = "")]
    width: u16,
    #[arg(long, help = "")]
    height: u16,
    #[arg(long, short = 'c', help = "")]
    particles_count: u16,
    #[arg(long, short = 'g', default_value = "0.05", help = "")]
    gravity_strength: String,
    #[arg(long, default_value = "0.01", help = "")]
    time_step: String,
}

fn main() {
    let mut display;
    let mut simulation_engine;

    {
        let config = <Config as clap::Parser>::parse();
        let gravity_strength = {
            match config.gravity_strength.parse::<f64>() {
                Ok(g) => g,
                Err(e) => {
                    println!("Could not parse option 'gravity-strength' to float type: {}", e);
                    return;
                }
            }
        };

        let time_step = {
            match config.time_step.parse::<f64>() {
                Ok(g) => g,
                Err(e) => {
                    println!("Could not parse option 'time-step' to float type: {}", e);
                    return;
                }
            }
        };

        match simulator::Engine::create(config.width as usize, config.height as usize, config.particles_count as usize, gravity_strength, time_step) {
            Ok(engine) => { simulation_engine = engine; } 
            Err(e) => {
                println!("Error: {}", e);
                return;
            }
        }

        display = display::Display::create(config.width, config.height);
    }

    loop {
        if crossterm::event::poll(Duration::from_secs(0)).unwrap() {
            if let Event::Key(_) = event::read().unwrap() { break; }
        }
        simulation_engine.update();
        display.draw(simulation_engine.get_display_frame());
        thread::sleep(Duration::from_millis(10));
    }
}
