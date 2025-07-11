mod cli;
mod scheduler;

#[tokio::main]
async fn main() {
    cli::print_help();

    let mut scheduler = scheduler::Scheduler::create();

    while let Some(cmd) = cli::wait_for_command() {
        scheduler.handle_command(cmd).await;
    }

    println!("timer scheduler stopped.")
}
