use crate::packet::{MacAddress, Packet, Protocol, EtherType};

use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, AtomicI64, Ordering};

use ahash::AHasher;
use chrono::Utc;
use dashmap::DashMap;

struct ConnectionData {
    local_address: IpAddr,
    remote_address: IpAddr,
    local_port: u16,
    remote_port: u16,
    remote_mac: MacAddress,
    tcp_packets: AtomicUsize,
    tcp_payload: AtomicUsize,
    udp_packets: AtomicUsize,
    udp_payload: AtomicUsize,
    total_bytes: AtomicUsize
}

pub(super) struct Entry {
    key: MacAddress,
    connections_data: DashMap<u64, ConnectionData>,
    total_packets: AtomicUsize,
    total_bytes: AtomicUsize,
    last_updated: AtomicI64,
}

impl Entry {
    pub fn create(mac_address: MacAddress) -> Self {
        Entry {
            key: mac_address,
            connections_data: DashMap::new(),
            total_bytes: AtomicUsize::new(0),
            total_packets: AtomicUsize::new(0),
            last_updated: AtomicI64::new(Utc::now().timestamp_micros()),
        }
    }

    pub fn get_last_updated(&self) -> i64 {
        self.last_updated.load(Ordering::Relaxed)
    }

    pub fn get_key(&self) -> MacAddress {
        self.key
    }

    pub fn update(&self, packet: &Packet) -> bool {
        let mut hasher = AHasher::default();
        let host_is_remote;
        let src_port;
        let dst_port;
        let is_tcp;

        {
            let src_mac = packet.get_source_mac();
            if src_mac == self.key {
                host_is_remote = false;
                packet.get_destination_mac().hash(&mut hasher);
                match packet.get_ethertype() {
                    EtherType::Ipv4(hdr) => {
                        hdr.get_source_address().hash(&mut hasher);
                        hdr.get_destination_address().hash(&mut hasher);
                    }
                    EtherType::Ipv6(hdr) => {
                        hdr.get_source_address().hash(&mut hasher);
                        hdr.get_destination_address().hash(&mut hasher);
                    }
                }
                match packet.get_protocol() {
                    Some(Protocol::Tcp(hdr)) => {
                        src_port = hdr.get_source_port();
                        src_port.hash(&mut hasher);
                        dst_port = hdr.get_destination_port();
                        dst_port.hash(&mut hasher);
                        is_tcp = true;
                    }
                    Some(Protocol::Udp(hdr)) => {
                        src_port = hdr.get_source_port();
                        src_port.hash(&mut hasher);
                        dst_port = hdr.get_destination_port();
                        dst_port.hash(&mut hasher);
                        is_tcp = false;
                    }
                    None => {
                        return false;
                    }
                }
            }
            else {
                host_is_remote = true;
                src_mac.hash(&mut hasher);
                match packet.get_ethertype() {
                    EtherType::Ipv4(hdr) => {
                        hdr.get_destination_address().hash(&mut hasher);
                        hdr.get_source_address().hash(&mut hasher);
                    }
                    EtherType::Ipv6(hdr) => {
                        hdr.get_destination_address().hash(&mut hasher);
                        hdr.get_source_address().hash(&mut hasher);   
                    }
                }
                match packet.get_protocol() {
                    Some(Protocol::Tcp(hdr)) => {
                        dst_port = hdr.get_destination_port();
                        dst_port.hash(&mut hasher);
                        src_port = hdr.get_source_port();
                        src_port.hash(&mut hasher);
                        is_tcp = true;
                    }
                    Some(Protocol::Udp(hdr)) => {
                        dst_port = hdr.get_destination_port();
                        dst_port.hash(&mut hasher);
                        src_port = hdr.get_source_port();
                        src_port.hash(&mut hasher);
                        is_tcp = false;
                    }
                    None => {
                        return false;
                    }
                }
            }
        }

        let connection_hash = hasher.finish();
        let packet_len = packet.get_length();
        let payload_len = packet.get_payload().len();

        self.connections_data.entry(connection_hash)
            .and_modify(|conn_data| {
            if is_tcp {
                conn_data.tcp_packets.fetch_add(1, Ordering::Relaxed);
                if payload_len > 0 {
                    conn_data.tcp_payload.fetch_add(payload_len, Ordering::Relaxed);
                }   
            }
            else {
                conn_data.udp_packets.fetch_add(1, Ordering::Relaxed);
                if payload_len > 0 {
                    conn_data.udp_payload.fetch_add(payload_len, Ordering::Relaxed);
                }   
            }
            conn_data.total_bytes.fetch_add(packet_len, Ordering::Relaxed);

        }).or_insert_with(|| {
            ConnectionData {
                remote_mac: if host_is_remote { packet.get_source_mac() } else { packet.get_destination_mac() },
                local_address: if host_is_remote { packet.get_destination_address() } else { packet.get_source_address() },
                remote_address: if host_is_remote {  packet.get_source_address() } else { packet.get_destination_address() },
                local_port: if host_is_remote { dst_port } else { src_port },
                remote_port: if host_is_remote { src_port } else { dst_port },
                tcp_packets: if is_tcp { AtomicUsize::new(1) } else { AtomicUsize::new(0) },
                udp_packets: if is_tcp { AtomicUsize::new(0) } else { AtomicUsize::new(1) },
                tcp_payload: if is_tcp { AtomicUsize::new(payload_len) } else { AtomicUsize::new(0) },
                udp_payload: if is_tcp { AtomicUsize::new(0) } else { AtomicUsize::new(payload_len) },
                total_bytes: AtomicUsize::new(packet_len)
            }
        });

        self.total_packets.fetch_add(1, Ordering::Relaxed);
        self.total_bytes.fetch_add(packet_len, Ordering::Relaxed);
        self.last_updated.store(Utc::now().timestamp_micros(), Ordering::Release);
        true
    }
}

impl Display for Entry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let connections_data_info = self.connections_data.iter().map(|el| format!("{}", el.value())).collect::<Vec<_>>();
        write!(f, "Mac: {:?}: total packets: {} total bytes: {}, last updated: {}. Connections:\n{}",
            self.key, self.total_packets.load(Ordering::Relaxed), self.total_bytes.load(Ordering::Relaxed), self.last_updated.load(Ordering::Relaxed), connections_data_info.join("\n"))
    }
}

impl Display for ConnectionData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: local: {}:{}, remote: {}:{}, total_bytes: {}, tcp_packets: {}, tcp_payload: {}, udp_packets: {}, udp_payload: {}",
            self.remote_mac, self.local_address, self.local_port, self.remote_address, self.remote_port, self.total_bytes.load(Ordering::Relaxed),
            self.tcp_packets.load(Ordering::Relaxed), self.tcp_payload.load(Ordering::Relaxed), self.udp_packets.load(Ordering::Relaxed), self.udp_payload.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        if self.key != other.key ||
        self.total_packets.load(Ordering::Relaxed) != other.total_packets.load(Ordering::Relaxed) ||
        self.total_bytes.load(Ordering::Relaxed) != other.total_bytes.load(Ordering::Relaxed) ||
        self.connections_data.len() != other.connections_data.len() {
            return false;
        }

        for el in self.connections_data.iter() {
            if let Some(other_el) = other.connections_data.get(el.key()) {
                let val = el.value();
                let other_val = other_el.value();

                if val.local_address != other_val.local_address ||
                    val.remote_address != other_val.remote_address ||
                    val.local_port != other_val.local_port ||
                    val.remote_port != other_val.remote_port ||
                    val.remote_mac != other_val.remote_mac ||
                    val.tcp_packets.load(Ordering::Relaxed) != other_val.tcp_packets.load(Ordering::Relaxed) ||
                    val.tcp_payload.load(Ordering::Relaxed) != other_val.tcp_payload.load(Ordering::Relaxed) ||
                    val.udp_packets.load(Ordering::Relaxed) != other_val.udp_packets.load(Ordering::Relaxed) ||
                    val.udp_payload.load(Ordering::Relaxed) != other_val.udp_payload.load(Ordering::Relaxed) ||
                    val.total_bytes.load(Ordering::Relaxed) != other_val.total_bytes.load(Ordering::Relaxed) {
                    return false;
                }
            }
            else {
                return false;
            }
        }

        true
    }

}

// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::test_init::get_logger;
    use crate::packet_source::pcap_reader::PcapReader;
    use crate::packet_source::PacketSource;
    use crate::packet_queue::ring_buffer::RingBuffer;
    use crate::packet_queue::PacketQueue;

    use std::net::{Ipv6Addr, Ipv4Addr};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    fn verify_connection_data(
        connections_data: &DashMap<u64, ConnectionData>,
        expected_remote_mac: MacAddress,
        expected_local_addr: IpAddr,
        expected_remote_addr: IpAddr,
        expected_local_port: u16,
        expected_remote_port: u16,
        expected_tcp_packets: usize,
        expected_tcp_payload: usize,
        expected_udp_packets: usize,
        expected_udp_payload: usize,
        expected_total_bytes: usize
    ) {
        let mut hasher = AHasher::default();
        expected_remote_mac.hash(&mut hasher);

        match expected_local_addr {
            IpAddr::V4(addr) => addr.hash(&mut hasher),
            IpAddr::V6(addr) => addr.hash(&mut hasher)
        }

        match expected_remote_addr {
            IpAddr::V4(addr) => addr.hash(&mut hasher),
            IpAddr::V6(addr) => addr.hash(&mut hasher)
        }

        expected_local_port.hash(&mut hasher);
        expected_remote_port.hash(&mut hasher);
        let key = hasher.finish();
        {
            let conn_data = connections_data.get(&key).expect("Data should be found");
            assert_eq!(conn_data.tcp_packets.load(Ordering::Relaxed), expected_tcp_packets);
            assert_eq!(conn_data.tcp_payload.load(Ordering::Relaxed), expected_tcp_payload);
            assert_eq!(conn_data.udp_packets.load(Ordering::Relaxed), expected_udp_packets);
            assert_eq!(conn_data.udp_payload.load(Ordering::Relaxed), expected_udp_payload);
            assert_eq!(conn_data.total_bytes.load(Ordering::Relaxed), expected_total_bytes);
        }
        connections_data.remove(&key);
    }

    fn verify_results_with_pattern(connections_data: &DashMap<u64, ConnectionData>, multiplier: usize) {
        verify_connection_data(
            &connections_data,
            [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36],
            IpAddr::V6(Ipv6Addr::new(0x2a02, 0x2a40, 0x58f2, 0xb700, 0x5ec6, 0xc2c3, 0xd651, 0x6277)),
            IpAddr::V6(Ipv6Addr::new(0x2a00, 0x9f00, 0x1002, 0x0000, 0x0000, 0x0000, 0x0000, 0x0053)),
            54284,
            53,
            0 * multiplier,
            0 * multiplier,
            1 * multiplier,
            58 * multiplier,
            120 * multiplier);

        verify_connection_data(
            &connections_data,
            [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36],
            IpAddr::V4(Ipv4Addr::new(192, 168, 0, 17)),
            IpAddr::V4(Ipv4Addr::new(178, 235, 153, 33)),
            57258,
            53,
            0 * multiplier,
            0 * multiplier,
            2 * multiplier,
            120 * multiplier,
            204 * multiplier);

        verify_connection_data(
            &connections_data,
            [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36],
            IpAddr::V4(Ipv4Addr::new(192, 168, 0, 17)),
            IpAddr::V4(Ipv4Addr::new(3, 174, 230, 121)),
            46106,
            443,
            1 * multiplier,
            0 * multiplier,
            0 * multiplier,
            0 * multiplier,
            74 * multiplier);

        verify_connection_data(
            &connections_data,
            [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36],
            IpAddr::V6(Ipv6Addr::new(0x2a02, 0x2a40, 0x58f2, 0xb700, 0x5ec6, 0xc2c3, 0xd651, 0x6277)),
            IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4802, 0x0034, 0x0000, 0x0000, 0x0000, 0x0036)),
            56152,
            443,
            0 * multiplier,
            0 * multiplier,
            1 * multiplier,
            31 * multiplier,
            93 * multiplier);
        
        verify_connection_data(
            &connections_data,
            [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36],
            IpAddr::V6(Ipv6Addr::new(0x2a02, 0x2a40, 0x58f2, 0xb700, 0x5ec6, 0xc2c3, 0xd651, 0x6277)),
            IpAddr::V6(Ipv6Addr::new(0x2a00, 0x9f00, 0x1002, 0x0000, 0x0000, 0x0000, 0x0000, 0x0053)),
            44134,
            53,
            0 * multiplier,
            0 * multiplier,
            1 * multiplier,
            135 * multiplier,
            197 * multiplier);
        
        verify_connection_data(
            &connections_data,
            [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36],
            IpAddr::V4(Ipv4Addr::new(192, 168, 0, 17)),
            IpAddr::V4(Ipv4Addr::new(18, 244, 146, 19)),
            56848,
            443,
            1 * multiplier,
            0 * multiplier,
            0 * multiplier,
            0 * multiplier,
            66 * multiplier);
        
        verify_connection_data(
            &connections_data,
            [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36],
            IpAddr::V6(Ipv6Addr::new(0x2a02, 0x2a40, 0x58f2, 0xb700, 0x5ec6, 0xc2c3, 0xd651, 0x6277)),
            IpAddr::V6(Ipv6Addr::new(0x2a06, 0x98c1, 0x3121, 0x0000, 0x0000, 0x0000, 0x0000, 0x000b)),
            55271,
            443,
            0 * multiplier,
            0 * multiplier,
            2 * multiplier,
            526 * multiplier,
            650 * multiplier);
        
        verify_connection_data(
            &connections_data,
            [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36],
            IpAddr::V4(Ipv4Addr::new(192, 168, 0, 17)),
            IpAddr::V4(Ipv4Addr::new(13, 227, 146, 90)),
            47954,
            443,
            2 * multiplier,
            4284 * multiplier,
            0 * multiplier,
            0 * multiplier,
            4416 * multiplier);
        
        verify_connection_data(
            &connections_data,
            [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36],
            IpAddr::V6(Ipv6Addr::new(0x2a02, 0x2a40, 0x58f2, 0xb700, 0x5ec6, 0xc2c3, 0xd651, 0x6277)),
            IpAddr::V6(Ipv6Addr::new(0x2a05, 0xd014, 0x04b8, 0x6f03, 0xeb35, 0x35e4, 0xb09a, 0x620d)),
            39936,
            443,
            1 * multiplier,
            0 * multiplier,
            0 * multiplier,
            0 * multiplier,
            86 * multiplier);

        verify_connection_data(
            &connections_data,
            [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36],
            IpAddr::V6(Ipv6Addr::new(0x2a02, 0x2a40, 0x58f2, 0xb700, 0x5ec6, 0xc2c3, 0xd651, 0x6277)),
            IpAddr::V6(Ipv6Addr::new(0x2a04, 0x4e42, 0x008e, 0x0000, 0x0000, 0x0000, 0x0000, 0x0503)),
            58152,
            443,
            2 * multiplier,
            0 * multiplier,
            0 * multiplier,
            0 * multiplier,
            172 * multiplier);

        assert!(connections_data.is_empty());

    }

    #[test]
    fn basic_entry_test() {
        let _ = get_logger();
        let (tx, _rx) = std::sync::mpsc::channel::<u8>();
        let queue: Arc<dyn PacketQueue> = Arc::new(RingBuffer::create(16).expect("RingBuffer should be created"));
        let pcap_reader: Box<dyn PacketSource> = Box::new(PcapReader::create(PathBuf::from("cache_test.pcap"), 10000, false).expect("Pcap reader should be created for test"));
        let _ = pcap_reader.run(1, Arc::clone(&queue), Arc::new(AtomicBool::new(true)), tx).join();
        let entry = Entry::create([0xAC, 0x12, 0x03, 0x16, 0x53, 0x6E]);
        while let Some(packet) = queue.pop() {
            entry.update(&packet);
        }

        assert_eq!(entry.key, [0xAC, 0x12, 0x03, 0x16, 0x53, 0x6E]);
        assert_eq!(entry.total_packets.load(Ordering::Relaxed), 14);
        assert_eq!(entry.total_bytes.load(Ordering::Relaxed), 6078);
        verify_results_with_pattern(&entry.connections_data, 1);
    }

    #[test]
    fn multiple_threads_to_one_entry() {
        let _ = get_logger();

        let queue: Arc<dyn PacketQueue> = Arc::new(RingBuffer::create(2048).expect("RingBuffer should be created"));
        for _ in 0..100 {   // pcap has 14 packets so lets multiply it by 100
            let (tx, _rx) = std::sync::mpsc::channel::<u8>();
            let pcap_reader: Box<dyn PacketSource> = Box::new(PcapReader::create(PathBuf::from("cache_test.pcap"), 10000, false).expect("Pcap reader should be created for test"));
            let _ = pcap_reader.run(1, Arc::clone(&queue), Arc::new(AtomicBool::new(true)), tx).join();
        }
        
        let entry = Arc::new(Entry::create([0xAC, 0x12, 0x03, 0x16, 0x53, 0x6E]));
        let start_popping = Arc::new(AtomicBool::new(false));
        let threads_num = 10;
        let mut joiners = Vec::with_capacity(threads_num);
        for _ in 0..threads_num {
            let e = Arc::clone(&entry);
            let q = Arc::clone(&queue);
            let start = Arc::clone(&start_popping);
            joiners.push(std::thread::spawn(move || {
                while !start.load(Ordering::Relaxed) {}
                while let Some(packet) = q.pop() {
                    e.update(&packet);
                }
            }));
        }
        
        start_popping.store(true, Ordering::Relaxed);

        for joiner in joiners {
            let _ = joiner.join();
        }

        assert_eq!(entry.key, [0xAC, 0x12, 0x03, 0x16, 0x53, 0x6E]);
        assert_eq!(entry.total_packets.load(Ordering::Relaxed), 1400);
        assert_eq!(entry.total_bytes.load(Ordering::Relaxed), 607800);
        verify_results_with_pattern(&entry.connections_data, 100);
    }
}

