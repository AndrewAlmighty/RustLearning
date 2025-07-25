mod process_scheduler_simulator;

use process_scheduler_simulator::types::Seconds;
use process_scheduler_simulator::Simulator;

#[derive(clap::Parser)]
pub struct Config {
    #[arg(long, help = "Number of processes which will be generated for simulation")]
    processes_count: usize,
    #[arg(long, help = "Available algorithm: fcfs (first come first served), srtf (shortest remaining time first), priority (highest priority first), rr (round robin)")]
    algorithm: String,
    #[arg(long, help = "Minimum time which process needs to execute in seconds (burst_time is random between min and max)")]
    burst_min: Seconds,
    #[arg(long, help = "Maximum time which process needs to execute in seconds (burst_time is random between min and max)")]
    burst_max: Seconds,
    #[arg(long, default_value_t = 1, help = "Number of threads for executing processes")]
    threads_count: u8,
    #[arg(long, default_value_t = 0, help = "Minimum time interval between process creation in seconds (generation_interval is random between min and max)")]
    min_interval: Seconds,
    #[arg(long, default_value_t = 0, help = "Maximum time interval between process creation in seconds(generation_interval is random between min and max)")]
    max_interval: Seconds,
    #[arg(long, default_value_t = 1, help = "round robin interval time - each process will be executed in T seconds chunks. Default is 1 second")]
    rr_interval: Seconds,
}

#[tokio::main]
async fn main() {
    let config = <Config as clap::Parser>::parse();
    
    match Simulator::create(config) {
        Ok(mut sim) => { sim.run().await; }
        Err(e) => { println!("Error: {}", e); }
    }
}
