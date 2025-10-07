use crate::packet_queue::PacketQueue;
use crate::packet::{Packet, Protocol};
use crate::log::*;

use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, AtomicU32, AtomicBool, Ordering};

pub struct RingBuffer {
    buffer: *mut MaybeUninit<Box<Packet>>,
    sequences: Vec<AtomicU32>,
    received_packets: AtomicUsize,
    dropped_packets: AtomicUsize,
    head: AtomicU32,
    tail: AtomicU32,
    capacity: u32,
    buffer_is_full: AtomicBool
}

impl RingBuffer {
    pub fn create(capacity: u32) -> Result<Self, String> {
        if capacity < 2 {
            return Err("Ring buffer capacity must be at least 2".to_string());
        }

        if !capacity.is_power_of_two() {
            return Err("Ring buffer capacity must be a number which is power of two".to_string());
        }

        let buffer: *mut MaybeUninit<Box<Packet>> = Box::into_raw(
            (0..capacity).map(|_| MaybeUninit::<Box<Packet>>::uninit())
            .collect::<Vec<_>>()
            .into_boxed_slice())
            as *mut MaybeUninit<Box<Packet>>;
        
        let sequences = (0..capacity).map(|n| AtomicU32::new(n)).collect::<Vec<_>>();

        log!("RingBuffer", log::Level::Debug, format!("RingBuffer is created. Capacity: {} packets", capacity));

        Ok(RingBuffer { 
            buffer: buffer,
            sequences: sequences,
            capacity: capacity,
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            received_packets: AtomicUsize::new(0),
            dropped_packets: AtomicUsize::new(0),
            buffer_is_full: AtomicBool::new(false)
         })
    }
}

unsafe impl Send for RingBuffer {}
unsafe impl Sync for RingBuffer {}

impl PacketQueue for RingBuffer {
    fn push(&self, packet: Box<Packet>) -> bool {
        loop {
            let head = self.head.load(Ordering::Relaxed);
            let idx = (head & (self.capacity - 1)) as usize;
            let seq = self.sequences.get(idx).expect(format!("There should be a sequence number. Idx: {}", idx).as_str());
            let diff = (seq.load(Ordering::Acquire) as isize) - (head as isize);

            if diff == 0 {
                let next_head = head.wrapping_add(1);
                if self.head.compare_exchange_weak(head, next_head, Ordering::AcqRel, Ordering::Relaxed).is_err() {
                    continue;
                }

                log!("RingBuffer", log::Level::Trace, format!("Received new packet. Id: {}. {} -> {}. Protocol: {}", 
                    packet.get_id(),
                    packet.get_source_address(),
                    packet.get_destination_address(),
                    match packet.get_protocol() {
                        None => "None".to_string(),
                        Some(Protocol::Tcp(_)) => "TCP".to_string(),
                        Some(Protocol::Udp(_)) => "UDP".to_string()
                    }));

                unsafe {
                    self.buffer.add(idx).write(MaybeUninit::new(packet));
                }

                seq.store(next_head, Ordering::Release);
                self.received_packets.fetch_add(1, Ordering::Relaxed);
                return true;
            }
            else if diff < 0 {
                if let Ok(false) = self.buffer_is_full.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed) {
                    log!("RingBuffer", log::Level::Debug, "Ring Buffer reached maximum capacity.".to_string());
                }
                self.dropped_packets.fetch_add(1, Ordering::Relaxed);
                return false;
            }
            else {
                continue;
            }
        }
    }

    fn pop(&self) -> Option<Box<Packet>> {
        loop {
            let tail = self.tail.load(Ordering::Relaxed);
            let next_tail = tail.wrapping_add(1);
            let idx = (tail & (self.capacity - 1)) as usize;
            let seq = self.sequences.get(idx).expect(format!("There should be a sequence number. Idx: {}", idx).as_str());
            
            if seq.load(Ordering::Acquire) != next_tail {
                return None;
            }

            if self.tail.compare_exchange_weak(tail, next_tail, Ordering::AcqRel, Ordering::Relaxed).is_ok() {
                let packet = unsafe {
                    self.buffer.add(idx).read().assume_init()
                };

                seq.store(tail.wrapping_add(self.capacity), Ordering::Release);

                if self.buffer_is_full.load(Ordering::Relaxed) {
                    self.buffer_is_full.store(false, Ordering::Relaxed);
                }

                return Some(packet);
            }
        }
    }

    fn get_queued_packets_count(&self) -> usize {
        self.received_packets.load(Ordering::Relaxed)
    }

    fn get_dropped_packets_count(&self) -> usize {
        self.dropped_packets.load(Ordering::Relaxed)
    }
}

impl Drop for RingBuffer {
    fn drop(&mut self) {
        unsafe {
            while let Some(_) = self.pop() {}
            drop(Box::from_raw(std::slice::from_raw_parts_mut(self.buffer, self.capacity as usize)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::test_init::get_logger;

    use std::sync::Arc;
    
    fn create_packet() -> Box<Packet> {
        Box::new(Packet::create(vec![
            0x01, 0x00, 0x5e, 0x00, 0x00, 0xfb, 0xac, 0x12, 0x03, 0x16, 0x53, 0x6e, 0x08, 0x00, 0x45, 0x00,
            0x00, 0x49, 0xaf, 0x78, 0x40, 0x00, 0xff, 0x11, 0x2a, 0x76, 0xc0, 0xa8, 0x00, 0x11, 0xe0, 0x00,
            0x00, 0xfb, 0x14, 0xe9, 0x14, 0xe9, 0x00, 0x35, 0xa1, 0xfb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x5f, 0x69, 0x70, 0x70, 0x04, 0x5f, 0x74, 0x63, 0x70,
            0x05, 0x6c, 0x6f, 0x63, 0x61, 0x6c, 0x00, 0x00, 0x0c, 0x00, 0x01, 0x05, 0x5f, 0x69, 0x70, 0x70,
            0x73, 0xc0, 0x11, 0x00, 0x0c, 0x00, 0x01]).expect("Packet should be created"))
    }

    #[test]
    fn ring_buffer_basic_test() {
        let _ = get_logger();
        let ring_buffer_creation = RingBuffer::create(8);
        assert!(ring_buffer_creation.is_ok());
        let ring_buffer = ring_buffer_creation.unwrap();
        assert_eq!(ring_buffer.head.load(Ordering::Relaxed), 0);
        assert_eq!(ring_buffer.tail.load(Ordering::Relaxed), 0);
        assert!(!ring_buffer.buffer_is_full.load(Ordering::Relaxed));
        assert_eq!(ring_buffer.capacity, 8);

        ring_buffer.push(create_packet());
        assert_eq!(ring_buffer.head.load(Ordering::Relaxed), 1);
        assert_eq!(ring_buffer.tail.load(Ordering::Relaxed), 0);
        assert!(!ring_buffer.buffer_is_full.load(Ordering::Relaxed));
        assert_eq!(ring_buffer.capacity, 8);

        let _ = ring_buffer.pop();
        assert_eq!(ring_buffer.head.load(Ordering::Relaxed), 1);
        assert_eq!(ring_buffer.tail.load(Ordering::Relaxed), 1);
        assert_eq!(ring_buffer.capacity, 8);
        assert!(!ring_buffer.buffer_is_full.load(Ordering::Relaxed));
    
        for _ in 0..8 {
            assert!(ring_buffer.push(create_packet()));
        }

        assert!(!ring_buffer.push(create_packet()));
        assert!(ring_buffer.buffer_is_full.load(Ordering::Relaxed));
        assert_eq!(ring_buffer.head.load(Ordering::Relaxed), 9);
        assert_eq!(ring_buffer.tail.load(Ordering::Relaxed), 1);

        for _ in 0..8 {
            assert!(ring_buffer.pop().is_some());
            assert!(!ring_buffer.buffer_is_full.load(Ordering::Relaxed));
        }

        assert!(ring_buffer.pop().is_none());
        assert_eq!(ring_buffer.head.load(Ordering::Relaxed), 9);
        assert_eq!(ring_buffer.tail.load(Ordering::Relaxed), 9);
    }

    #[test]
    fn ring_buffer_multiple_producers_do_not_exceeds_limit_test() {
        let _ = get_logger();
        let buffer_size = 4096;
        let total_packets_to_push = 6000;
        let buffer = Arc::new(RingBuffer::create(buffer_size).expect("Buffer should be created"));
        let threads_num = 6;
        assert_eq!(total_packets_to_push % threads_num, 0);
        let mut thread_joiners = Vec::with_capacity(threads_num);
        let running_flag = Arc::new(AtomicBool::new(false));
        let packets_per_thread = total_packets_to_push / threads_num;

        for _ in 0..threads_num {
            let packets_to_create = packets_per_thread;            
            let r = Arc::clone(&running_flag);
            let b = Arc::clone(&buffer);
            thread_joiners.push(std::thread::spawn(move || {
                while !r.load(Ordering::Relaxed) {}
                for _ in 0..packets_to_create {
                    b.push(create_packet());
                }
            }));
        }

        running_flag.store(true, Ordering::Relaxed);
        for joiner in thread_joiners {
            let _ = joiner.join();
        }

        assert_eq!(buffer.get_queued_packets_count(), buffer_size as usize);
        assert_eq!(buffer.get_dropped_packets_count(), total_packets_to_push - buffer_size as usize);
        assert!(buffer.buffer_is_full.load(Ordering::Relaxed));
        assert_eq!(buffer.head.load(Ordering::Relaxed), buffer_size);
        assert_eq!(buffer.tail.load(Ordering::Relaxed), 0);
    }

    fn ring_buffer_no_double_consume_check(buffer_size: u32, total_packets_to_push: usize, producers_num: usize, consumers_num: usize, expected_dropped_packets: bool) {
        let _ = get_logger();
        let buffer = Arc::new(RingBuffer::create(buffer_size).expect("Buffer should be created"));
        assert_eq!(total_packets_to_push % producers_num, 0);
        let mut producer_joiners = Vec::with_capacity(producers_num);
        let mut consumer_joiners = Vec::with_capacity(consumers_num);
        let producers_running_flag = Arc::new(AtomicBool::new(false));
        let consumers_running_flag = Arc::new(AtomicBool::new(true));
        let consumers_ids_set_in_mutex = Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
        let producers_ids_set_in_mutex = Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
        let packets_per_thread = total_packets_to_push / producers_num;
        let successfull_push_count = Arc::new(AtomicUsize::new(0));

        for _ in 0..consumers_num {
            let i_s = Arc::clone(&consumers_ids_set_in_mutex);
            let r = Arc::clone(&consumers_running_flag);
            let b = Arc::clone(&buffer);
            consumer_joiners.push(std::thread::spawn(move || {
                loop {
                    if let Some(packet) = b.pop() {
                        assert!(i_s.lock().unwrap().insert(packet.get_id()));
                    }
                    else if !r.load(Ordering::Relaxed) {
                        break;
                    }
                }
            }));
        }

        for _ in 0..producers_num {
            let i_s = Arc::clone(&producers_ids_set_in_mutex);
            let spc = Arc::clone(&successfull_push_count);
            let packets_to_create = packets_per_thread;            
            let r = Arc::clone(&producers_running_flag);
            let b = Arc::clone(&buffer);
            let check_push = !expected_dropped_packets;
            producer_joiners.push(std::thread::spawn(move || {
                while !r.load(Ordering::Relaxed) {}
                for _ in 0..packets_to_create {
                    let packet = create_packet();
                    let packet_id = packet.get_id();
                    if check_push { 
                        assert!(b.push(packet));
                        spc.fetch_add(1, Ordering::Relaxed);
                        i_s.lock().unwrap().insert(packet_id);
                    }
                    else {
                        if b.push(packet) {
                            spc.fetch_add(1, Ordering::Relaxed);
                            i_s.lock().unwrap().insert(packet_id);
                        }
                    }
                }
            }));
        }

        
        producers_running_flag.store(true, Ordering::Relaxed);
    
        for joiner in producer_joiners {
            let _ = joiner.join();
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
        consumers_running_flag.store(false, Ordering::Relaxed);

        for joiner in consumer_joiners {
            let _ = joiner.join();
        }

        let c_ids_set = consumers_ids_set_in_mutex.lock().unwrap();
        let p_ids_set = producers_ids_set_in_mutex.lock().unwrap();
        let dropped_packets_count = buffer.get_dropped_packets_count();
        assert_eq!(c_ids_set.len(), total_packets_to_push - dropped_packets_count);
        assert_eq!(p_ids_set.len(), total_packets_to_push - dropped_packets_count);
        assert_eq!(*p_ids_set, *c_ids_set);

        assert_eq!(total_packets_to_push, buffer.get_queued_packets_count().wrapping_add(dropped_packets_count));
        assert_eq!(buffer.head.load(Ordering::Relaxed), successfull_push_count.load(Ordering::Relaxed) as u32);

        if expected_dropped_packets {
            assert!(dropped_packets_count > 0);
            assert_eq!(dropped_packets_count, total_packets_to_push - successfull_push_count.load(Ordering::Relaxed));
            assert_eq!(buffer.tail.load(Ordering::Relaxed), (total_packets_to_push - dropped_packets_count) as u32);
        }
        else {
            assert_eq!(buffer.head.load(Ordering::Relaxed), total_packets_to_push as u32);
            assert_eq!(successfull_push_count.load(Ordering::Relaxed), total_packets_to_push);
        }
    }

    #[test]
    fn ring_buffer_small_capacity_test() {
        ring_buffer_no_double_consume_check(4, 2000, 2, 1, true);
    }

    #[test]
    fn ring_buffer_multiple_producers_no_double_consume_packets_amount_below_limit_test() {
        ring_buffer_no_double_consume_check(4096, 4000, 4, 1, false);
    }

    #[test]
    fn ring_buffer_multiple_producers_no_double_consume_packets_amount_exact_as_buffer_size_test() {
        ring_buffer_no_double_consume_check(8192, 8192, 8, 1, false);
    }

    #[test]
    fn ring_buffer_multiple_producers_no_double_consume_packets_amount_exceeds_buffer_size_test() {
        ring_buffer_no_double_consume_check(1024, 12288, 8, 1, true);
    }

    #[test]
    fn ring_buffer_multiple_producers_multiple_consumers() {
        ring_buffer_no_double_consume_check(128, 16384, 4, 4, true);
    }
}