mod cli;
mod vfs;

fn main() {
    cli::print_introduction();
    let mut filesystem = vfs::VirtualFilesystem::new();

    loop {
        filesystem.print_path();
        let cmd = cli::wait_for_command();

        match cmd {
            Some(cmd) => {
                if let Err(err) = filesystem.handle_command(cmd) {
                    println!("{}", err);
                } 
            }
            None => { break; }
        }
    }

    println!("\n\nClosing virtual filesystem simulator");
}
