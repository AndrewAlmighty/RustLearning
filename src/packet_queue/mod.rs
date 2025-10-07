pub mod dummy_queue;
pub mod ring_buffer;

use crate::packet::Packet;

pub trait PacketQueue: Send + Sync {
    fn get_queued_packets_count(&self) -> usize;
    fn get_dropped_packets_count(&self) -> usize;
    fn push(&self, packet: Box<Packet>) -> bool;
    fn pop(&self) -> Option<Box<Packet>>;
}