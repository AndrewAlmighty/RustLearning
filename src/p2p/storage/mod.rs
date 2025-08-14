mod manager;
pub(super) mod file;
pub(super) mod message;

use crate::StatusSender;
use crate::reporting::display::LogSender;

use manager::{Manager, NodeMessageReceiver, StorageMessageSender};

use std::path::PathBuf;

pub async fn run(log_sender: LogSender, storage_status_sender: StatusSender, storage_message_sender: StorageMessageSender, node_message_receiver: NodeMessageReceiver, shared_files_dir: PathBuf) -> Result<(), std::io::Error> {
    let manger = Manager::create(log_sender, storage_status_sender, storage_message_sender, node_message_receiver, shared_files_dir).await?;
    manger.start();
    Ok(())
}