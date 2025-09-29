pub mod dummy_queue;

use crate::packet::Packet;

pub trait PacketQueue: Send + Sync {
    fn get_queued_packets_count(&self) -> usize;
    fn push(&self, packet: Packet);
}