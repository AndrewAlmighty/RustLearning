pub mod packet_factory;

use crate::packet_queue::PacketQueue;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;

pub trait PacketSource {
    fn run(self, name: String, packet_queue: Arc<dyn PacketQueue>, running: Arc<AtomicBool>) -> JoinHandle<()>;
}