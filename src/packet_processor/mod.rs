use crate::log::*;
use crate::packet_queue::PacketQueue;
use crate::packet::{Packet};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Instant, Duration};
use std::thread::JoinHandle;

pub struct PacketProcessor {
    id: u8
}

impl PacketProcessor {
    pub fn create(id: u8) -> Self {
        PacketProcessor { 
            id: id
        }
    }

    pub fn get_id(&self) -> u8 {
        self.id
    }

    fn handle_packet(&self, packet: Box<Packet>) {
        log!("PacketProcessor", log::Level::Trace, format!("[ID:{}] handling packet with ID: {}.", self.id, packet.get_id()));
    }

    pub fn run(self, packet_queue: Arc<dyn PacketQueue>, running: Arc<AtomicBool>, is_finished_sender: Sender<u8>) -> JoinHandle<()> {
        std::thread::spawn(move || {
            const REPORT_INTERVAL_IN_SECS: u64 = 60;
            let mut total_received_packets = 0usize;
            let mut last_received_packets_count = 0usize;
            let begin_time =  Instant::now();
            let mut last_report_time = begin_time.clone();

            log!("PacketProcessor", log::Level::Debug, format!("[ID:{}] Started work.", self.id));
    
            loop {
                if let Some(packet) = packet_queue.pop() {
                    self.handle_packet(packet);
                    total_received_packets += 1;
                }
                else if !running.load(Ordering::Relaxed) {
                    break;
                }
                else {
                    std::thread::sleep(Duration::from_millis(1));
                }

                let now = Instant::now();
                if now.duration_since(last_report_time) >= Duration::from_secs(REPORT_INTERVAL_IN_SECS) {
                    log!("PacketProcessor", log::Level::Info, format!(
                        "[ID:{}] received: {} packets, Current throughput: {:.2} packets per second.",
                        self.id, total_received_packets, (total_received_packets - last_received_packets_count) as f64 / REPORT_INTERVAL_IN_SECS as f64
                    ));

                    last_report_time = now;
                    last_received_packets_count = total_received_packets;
                }
            }

            log!("PacketProcessor", log::Level::Debug,
                format!("[ID:{}] Finished work. Received {} packets, Average throughput: {:.2} packets per second",
                self.id, total_received_packets, (total_received_packets as f64) / (Instant::now().duration_since(begin_time).as_secs() as f64)));

            if let Err(e) = is_finished_sender.send(self.id) {
                panic!("Pcap reader with ID: {} could not inform about finished work. Error: {}", self.id, e);
            }
        })
    }
}