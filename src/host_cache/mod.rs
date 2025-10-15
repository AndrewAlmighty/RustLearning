mod entry;

use crate::log::*;
use crate::packet::{MacAddress, Packet, mac_to_string};
use crate::host_cache::entry::Entry;

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

use ahash::AHasher;
use chrono::Utc;

pub struct HostCache {
    buckets: *mut *mut Entry,
    capacity: usize,
    expiration_time: i64,   //microseconds
    current_size: AtomicUsize,
    hits_count: AtomicUsize,
    miss_count: AtomicUsize,
    rejected_count: AtomicUsize,
    current_updaters: AtomicU8,
    locked: AtomicBool
}

impl HostCache {
    pub fn create(capacity: usize, expiration_time_in_seconds: u8) -> Result<Self, String> {
        if capacity == 0 {
            return Err("Host cache capacity cannot be 0".to_string());
        }

        if !capacity.is_power_of_two() {
            return Err("Host cache capacity must be power of two".to_string());
        }

        if expiration_time_in_seconds == 0 {
             return Err("Entry's expiration time must be at least one second".to_string());
        }

        let mut buckets = vec![std::ptr::null_mut::<Entry>(); capacity];
        let first_bucket = buckets.as_mut_ptr();
        std::mem::forget(buckets);

        Ok(
            HostCache {
                buckets: first_bucket,
                capacity: capacity,
                expiration_time: (expiration_time_in_seconds as i64) * 1_000_000,
                current_size: AtomicUsize::new(0),
                hits_count: AtomicUsize::new(0),
                miss_count: AtomicUsize::new(0),
                rejected_count: AtomicUsize::new(0),
                current_updaters: AtomicU8::new(0),
                locked: AtomicBool::new(false)
            }
        )
    }

    fn calculate_idx_for_mac(&self, mac: MacAddress) -> usize {
        let mut hasher = AHasher::default();
        mac.hash(&mut hasher);
        (hasher.finish() as usize) & (self.capacity - 1)
    }

    pub fn extract_packet_data(&self, packet: Box<Packet>) {
        loop {
            if self.locked.load(Ordering::Acquire) {
                std::hint::spin_loop();
                continue;
            }

            assert!(self.current_updaters.fetch_add(1, Ordering::Relaxed) < 255, "No more threads than 255 are allowed");
            if self.locked.load(Ordering::Acquire) {
                self.current_updaters.fetch_sub(1, Ordering::Relaxed);
                std::hint::spin_loop();
                continue;
            }

            break;
        }

        let src_mac = packet.get_source_mac();
        let dst_mac = packet.get_destination_mac();
        let src_mac_idx = self.calculate_idx_for_mac(src_mac);
        let dst_mac_idx = self.calculate_idx_for_mac(dst_mac);
        let entry_src_mac_updated = self.update_mac_connection_data(src_mac_idx, src_mac, &packet);
        let entry_dst_mac_updated = self.update_mac_connection_data(dst_mac_idx, dst_mac, &packet);
        self.current_updaters.fetch_sub(1, Ordering::Relaxed);

        if !entry_src_mac_updated || !entry_dst_mac_updated {
            while self.locked.swap(true, Ordering::AcqRel) { std::hint::spin_loop(); }
            while self.current_updaters.load(Ordering::Acquire) != 0 { std::hint::spin_loop() ;}

            if !entry_src_mac_updated {
                self.update_or_try_insert(src_mac_idx, src_mac, &packet);
            }

            if !entry_dst_mac_updated {
                self.update_or_try_insert(dst_mac_idx, dst_mac, &packet);
            }
            
            self.locked.store(false, Ordering::Release);
        }
    }


    fn update_or_try_insert(&self, original_idx: usize, mac: MacAddress, packet: &Packet) {
        if self.update_mac_connection_data(original_idx, mac, &packet) {
            return;
        }

        let mut idx = original_idx;
        let mut oldest_timestamp = i64::MAX;
        let mut idx_with_oldest_timestamp = 0usize;

        loop {
            unsafe {
                let slot = self.buckets.add(idx);
                let entry_ptr = *slot;
                if entry_ptr.is_null() {
                    let entry = Box::new(Entry::create(mac));
                    entry.update(&packet);
                    *slot = Box::into_raw(entry);
                    self.miss_count.fetch_add(1, Ordering::Relaxed);
                    let prev_size = self.current_size.fetch_add(1, Ordering::Relaxed);
                    debug_assert!(prev_size < self.capacity);
                    log!("HostCache", log::Level::Debug, format!("Created new entry for MAC: {}, idx: {}", mac_to_string(mac), idx));
                    return;
                }
                else {
                    let last_updated = (*entry_ptr).get_last_updated();
                    if last_updated < oldest_timestamp {
                        oldest_timestamp = last_updated;
                        idx_with_oldest_timestamp = idx;
                    }
                }
            }

            idx = (idx + 1) & (self.capacity - 1);
            if idx == original_idx {
                break;
            }
        }

        let current_time = Utc::now().timestamp_micros();
        if current_time.saturating_sub(oldest_timestamp) > self.expiration_time {
            unsafe {
                let slot = self.buckets.add(idx_with_oldest_timestamp);
                let old_entry_ptr = *slot;
                assert!(!old_entry_ptr.is_null());
                log!("HostCache", log::Level::Debug, format!("Removing entry with mac: {:?}. It's idx: {}, timestamp {}", mac_to_string((*old_entry_ptr).get_key()), idx_with_oldest_timestamp, oldest_timestamp));
                std::ptr::drop_in_place(old_entry_ptr);
                std::ptr::write(old_entry_ptr, Entry::create(mac));
                (*old_entry_ptr).update(&packet);
                self.miss_count.fetch_add(1, Ordering::Relaxed);
                log!("HostCache", log::Level::Debug, format!("Created new entry for MAC: {:?}, idx: {} after removing old one.", mac_to_string(mac), idx));
                return;
            }
        }
        else {
            self.rejected_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn update_mac_connection_data(&self, original_idx: usize, mac: MacAddress, packet: &Packet) -> bool {
        let mut idx = original_idx;

        loop {
            unsafe {
                let entry_ptr = *self.buckets.add(idx);
                if !entry_ptr.is_null() {
                    let entry = &*entry_ptr;
                    if entry.get_key() == mac {
                        entry.update(packet);
                        self.hits_count.fetch_add(1, Ordering::Relaxed);
                        log!("HostCache", log::Level::Trace, format!("Updated entry with mac: {}. It's idx: {}", mac_to_string(mac), idx));
                        return true;
                    }
                }
            }

            idx = (idx + 1) & (self.capacity - 1);
            if idx == original_idx {
                break;
            }
        }

        false
    }

    pub fn print_cache_data(&self) {
        while self.locked.swap(true, Ordering::AcqRel) { std::hint::spin_loop(); }
        while self.current_updaters.load(Ordering::Acquire) != 0 { std::hint::spin_loop(); }

        let current_size = self.current_size.load(Ordering::Relaxed);
        let mut entries_data = Vec::with_capacity(current_size);
        for idx in 0..current_size {
            unsafe {
                let entry_ptr = *self.buckets.add(idx);
                if !entry_ptr.is_null() {
                    entries_data.push(format!("{}", *entry_ptr));
                }
            }
        }

        entries_data.shrink_to_fit();
        log!("HostCache", log::Level::Info, format!("\nCurrent cache status\nHits: {}\nMisses: {}\nRejections: {}\nEntries:\n{}\n",
            self.hits_count.load(Ordering::Relaxed), self.miss_count.load(Ordering::Relaxed), self.rejected_count.load(Ordering::Relaxed), entries_data.join("\n")));

        self.locked.store(false, Ordering::Release);
    }
}

impl Drop for HostCache {
    fn drop(&mut self) {
        for idx in 0..self.capacity {
            unsafe {
                let entry_ptr = *self.buckets.add(idx);
                if !entry_ptr.is_null() {
                    std::ptr::drop_in_place(entry_ptr);
                }
            }
        }
        unsafe {
            let _ = Vec::from_raw_parts(self.buckets, self.capacity, self.capacity);
        }
    }
}

unsafe impl Send for HostCache {}
unsafe impl Sync for HostCache {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::test_init::get_logger;
    use crate::packet_source::pcap_reader::PcapReader;
    use crate::packet_source::packet_factory::PacketFactory;
    use crate::packet_source::PacketSource;
    use crate::packet_queue::ring_buffer::RingBuffer;
    use crate::packet_queue::PacketQueue;

    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn basic_host_cache_test() {
        let _ = get_logger();
        let cache = Arc::new(HostCache::create(8, 100).expect("Cache should be created"));

        {
            let queue: Arc<dyn PacketQueue> = Arc::new(RingBuffer::create(2048).expect("RingBuffer should be created"));
            for _ in 0..100 {   // pcap has 14 packets so lets multiply it by 100
                let (tx, _rx) = std::sync::mpsc::channel::<u8>();
                let pcap_reader: Box<dyn PacketSource> = Box::new(PcapReader::create(PathBuf::from("cache_test.pcap"), 10000, false).expect("Pcap reader should be created for test"));
                let _ = pcap_reader.run(1, Arc::clone(&queue), Arc::new(AtomicBool::new(true)), tx).join();
            }

            
            let start_popping = Arc::new(AtomicBool::new(false));
            let threads_num = 10;
            let mut joiners = Vec::with_capacity(threads_num);
            for _ in 0..threads_num {
                let c = Arc::clone(&cache);
                let q = Arc::clone(&queue);
                let start = Arc::clone(&start_popping);
                joiners.push(std::thread::spawn(move || {
                    while !start.load(Ordering::Relaxed) {}
                    while let Some(packet) = q.pop() {
                        c.extract_packet_data(packet);
                    }
                }));
            }
            
            start_popping.store(true, Ordering::Relaxed);

            for joiner in joiners {
                let _ = joiner.join();
            }
        }

        assert_eq!(cache.hits_count.load(Ordering::Relaxed), 2798);
        assert_eq!(cache.miss_count.load(Ordering::Relaxed), 2);
        assert_eq!(cache.rejected_count.load(Ordering::Relaxed), 0);

        let create_entry = |mac: MacAddress| {
            let queue: Arc<dyn PacketQueue> = Arc::new(RingBuffer::create(2048).expect("RingBuffer should be created"));
            for _ in 0..100 {   // pcap has 14 packets so lets multiply it by 100
                let (tx, _rx) = std::sync::mpsc::channel::<u8>();
                let pcap_reader: Box<dyn PacketSource> = Box::new(PcapReader::create(PathBuf::from("cache_test.pcap"), 10000, false).expect("Pcap reader should be created for test"));
                let _ = pcap_reader.run(1, Arc::clone(&queue), Arc::new(AtomicBool::new(true)), tx).join();
            }

            let entry = Entry::create(mac);
            while let Some(packet) = queue.pop() {
                entry.update(&packet);
            }

            entry
        };

        let entry_1 = create_entry([0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36]);
        let entry_2 = create_entry([0xAC, 0x12, 0x03, 0x16, 0x53, 0x6E]);
        let mut entries_found = 0;

        for idx in 0..cache.capacity {
            unsafe {
                let entry_ptr = *cache.buckets.add(idx);
                if entry_ptr.is_null() {
                    continue;
                }

                entries_found += 1;
                if (*entry_ptr).get_key() == [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36] {
                    if *entry_ptr != entry_1 {
                        panic!("{}", format!("Entry from cache differs pattern! From cache:\n{}\n\nFrom pattern:\n{}", *entry_ptr, entry_1).as_str());
                    }
                }
                else if (*entry_ptr).get_key() == [0xAC, 0x12, 0x03, 0x16, 0x53, 0x6E] {
                    if *entry_ptr != entry_2 {
                        panic!("{}", format!("Entry from cache differs pattern! From cache:\n{}\n\nFrom pattern:\n{}", *entry_ptr, entry_1).as_str());
                    }
                }
                else {
                    panic!("{}", format!("Found entry: {:?}", (*entry_ptr).get_key()));
                }
            }
        }

        assert_eq!(entries_found, 2);
    }

    #[test]
    fn cache_tests_deletion_and_rejection() {
        let _ = get_logger();
        let cache = Arc::new(HostCache::create(8, 1).expect("Cache should be created"));
        let queue: Arc<dyn PacketQueue> = Arc::new(RingBuffer::create(256).expect("RingBuffer should be created"));
        let (tx, _rx) = std::sync::mpsc::channel::<u8>();
        let pcap_reader: Box<dyn PacketSource> = Box::new(PacketFactory::create(10000, 256, 8, 8, 1).expect("Pcap factory should be created for test"));
        let _ = pcap_reader.run(1, Arc::clone(&queue), Arc::new(AtomicBool::new(true)), tx).join();

        for _ in 0..128 {
            let packet = queue.pop().expect("There should be a packet");
            cache.extract_packet_data(packet);
        }

        {
            let miss_count = cache.miss_count.load(Ordering::Relaxed);
            let hits_count = cache.hits_count.load(Ordering::Relaxed);
            let rejected_count = cache.rejected_count.load(Ordering::Relaxed);
            assert_eq!(miss_count, 8);
            assert_eq!(256, miss_count + hits_count + rejected_count);
        }

        std::thread::sleep(std::time::Duration::from_millis(1100));

        for _ in 0..128 {
            let packet = queue.pop().expect("There should be a packet");
            cache.extract_packet_data(packet);
        }

        let miss_count = cache.miss_count.load(Ordering::Relaxed);
        let hits_count = cache.hits_count.load(Ordering::Relaxed);
        let rejected_count = cache.rejected_count.load(Ordering::Relaxed);
        assert!(miss_count >= 8);
        assert_eq!(512, miss_count + hits_count + rejected_count);
    }
}