pub mod packet_factory;
pub mod pcap_reader;

use crate::packet_queue::PacketQueue;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

pub trait PacketSource {
    fn run(self: Box<Self>, id: u8, packet_queue: Arc<dyn PacketQueue>, running: Arc<AtomicBool>, is_finished_sender: Sender<u8>) -> JoinHandle<()>;
}