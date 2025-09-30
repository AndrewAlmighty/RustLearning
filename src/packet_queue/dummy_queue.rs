use crate::packet_queue::PacketQueue;
use crate::packet_queue::Packet;
use crate::log::*;

use std::sync::atomic::{AtomicUsize, Ordering};

pub struct DummyQueue {
    received_packets: AtomicUsize
}

impl DummyQueue {
    #[allow(dead_code)]
    pub fn create() -> Self {
        DummyQueue { received_packets: AtomicUsize::new(0) }
    }
}

impl PacketQueue for DummyQueue {
    fn push(&self, packet: Packet) {
        log!("DummyQueue", log::Level::Trace, format!("Pushed a new packet to dummy queue. Id: {}, timestamp: {}", packet.get_id(), packet.get_timestamp()));
        self.received_packets.fetch_add(1, Ordering::Relaxed);
    }

    fn get_queued_packets_count(&self) -> usize {
        self.received_packets.load(Ordering::Relaxed)
    }
}