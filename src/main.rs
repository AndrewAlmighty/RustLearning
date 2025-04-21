mod cli;
mod contact_manager;

fn main() {
    cli::print_introduction();
    let mut cm = contact_manager::ContactManager::new();

    loop {
        let cmd = cli::wait_for_command();

        match cmd {
            Some(cmd) => {
                if let Err(err) = cm.handle_command(cmd) {
                    println!("{}", err);
                } 
            }
            None => { break; }
        }
    }

    println!("\n\nClosing ...");
}