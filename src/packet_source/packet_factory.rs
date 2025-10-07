use crate::packet_source::PacketSource;
use crate::packet_queue::PacketQueue;
use crate::packet::{Packet, ProtocolKind};
use crate::log::*;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::{Instant, Duration};

use rand::{
    prelude::IndexedRandom,
    Rng,
    rngs::ThreadRng};

const GENERATED_IPV4_HEADER_LEN: usize = 20;
const GENERATED_IPV6_HEADER_LEN: usize = 40;
const GENERATED_TCP_HEADER_LEN: usize = 20;
const GENERATED_UDP_HEADER_LEN: usize = 8;

struct Host {
    ip_bytes: Vec<u8>,
    mac_addresses: Vec<Vec<u8>>,
}

impl Host {
    fn create(ip_bytes: Vec<u8>, mac_addresses: Vec<Vec<u8>>) -> Self {
        let ip_bytes_len = ip_bytes.len();
        assert!(ip_bytes_len == 4 || ip_bytes_len == 16);
        assert!(!mac_addresses.is_empty());
        for mac in &mac_addresses {
            assert_eq!(mac.len(), 6);
        }

        Host { ip_bytes: ip_bytes, mac_addresses: mac_addresses }
    }

    fn get_ip_bytes(&self) -> Vec<u8> {
        self.ip_bytes.clone()
    }

    fn get_random_mac(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let mac_addresses_available_count = self.mac_addresses.len();
        if mac_addresses_available_count == 1 {
            self.mac_addresses[0].clone()
        }
        else {
            self.mac_addresses.choose(rng).expect("No values in host mac addresses!").clone()
        }
    }
}

pub struct PacketFactory {
    packets_limit: usize,
    creation_interval: Duration,
    ipv4_hosts: Vec<Host>,
    ipv6_hosts: Vec<Host>
}

impl PacketFactory {
    pub fn create(packets_per_second: u64, packets_limit: usize, ipv4_hosts_number: usize, ipv6_hosts_number: usize, macs_per_host: usize) -> Result<Self, String> {
        fn generate_mac_address(rng: &mut ThreadRng) -> Vec<u8> {
            let mut mac_bytes = Vec::<u8>::with_capacity(6);
            for _ in 0..6 {
                mac_bytes.push(rng.random_range(0..=255));
            }

            mac_bytes
        }

        fn generate_ip_address(rng: &mut ThreadRng, generate_ipv4: bool) -> Vec<u8> {
            let address_size: usize = if generate_ipv4 { 4 } else { 16 };
            let mut bytes = Vec::with_capacity(address_size);
            for _ in 0..address_size {
                bytes.push(rng.random_range(0..=255));
            }
            bytes
        }

        const PACKETS_PER_SECONDS_MAX_LIMIT: u64 = 10000;
        if packets_per_second > PACKETS_PER_SECONDS_MAX_LIMIT {
            return Err(format!("Cannot create PacketFactory - requested to create {} packets per second, which is above limit: {}", packets_per_second, PACKETS_PER_SECONDS_MAX_LIMIT));
        }

        if packets_per_second == 0 {
            return Err("Cannot create PacketFactory - packets per second must be bigger than 0".to_string());
        }

        if ipv4_hosts_number > 1000 {
            return Err("Cannot create PacketFactory - ipv4_hosts_number limit is 1000".to_string());
        }

        if ipv6_hosts_number > 1000 {
            return Err("Cannot create PacketFactory - ipv6_hosts_number limit is 1000".to_string());
        }

        if ipv4_hosts_number == 0 && ipv6_hosts_number == 0 {
            return Err("Cannot create PacketFactory - there must be available at least one host, ipv4 or ipv6".to_string());
        }

        if macs_per_host == 0 {
            return Err("Cannot create PacketFactory - every host should have at least one mac address".to_string());
        }

        if macs_per_host > 10 {
            return Err("Cannot create PacketFactory - macs_per_host limit is 10".to_string());
        }


        let mut rng = rand::rng();
        let mut ipv4_hosts: Vec<Host> = if ipv4_hosts_number == 0 { Vec::new() } else { Vec::with_capacity(ipv4_hosts_number) };
        let mut ipv6_hosts: Vec<Host> = if ipv6_hosts_number == 0 { Vec::new() } else { Vec::with_capacity(ipv6_hosts_number) };

        for _ in 0..ipv4_hosts_number {
            ipv4_hosts.push(Host::create(generate_ip_address(&mut rng, true), (0..macs_per_host).map(|_| generate_mac_address(&mut rng)).collect()));
        }

        for _ in 0..ipv6_hosts_number {
            ipv6_hosts.push(Host::create(generate_ip_address(&mut rng, false), (0..macs_per_host).map(|_| generate_mac_address(&mut rng)).collect()));
        }

        const MICROSECONDS_IN_SECOND: u64 = 1_000_000;
        let packet_creation_interval = MICROSECONDS_IN_SECOND / packets_per_second;
        Ok(PacketFactory {packets_limit: packets_limit, creation_interval: Duration::from_micros(packet_creation_interval), ipv4_hosts: ipv4_hosts, ipv6_hosts: ipv6_hosts })
    }

    fn generate_packet_bytes(&self, mut rng: &mut ThreadRng) -> Vec<u8> {
        let mut packet_bytes_size = 12usize; // 2x mac address + ethertype bytes

        let protocol_bytes;
        let protocol_kind;
        match rng.random_bool(0.5) {
            true => {
                packet_bytes_size += GENERATED_TCP_HEADER_LEN;
                protocol_kind = ProtocolKind::Tcp;
                protocol_bytes = self.generate_tcp_header_bytes(&mut rng);
            }
            false => {
                packet_bytes_size += GENERATED_UDP_HEADER_LEN;
                protocol_kind = ProtocolKind::Udp;
                protocol_bytes = self.generate_udp_header_bytes(&mut rng);
            }
        };

        let ethertype_header_bytes;
        let mac_1_bytes;
        let mac_2_bytes;
        
        if self.ipv4_hosts.is_empty() {
            packet_bytes_size += GENERATED_IPV6_HEADER_LEN;
            (ethertype_header_bytes, mac_1_bytes, mac_2_bytes) = self.generate_ipv6_header_bytes(protocol_kind, &mut rng);
        }
        else if self.ipv6_hosts.is_empty() {
            packet_bytes_size += GENERATED_IPV6_HEADER_LEN;
            (ethertype_header_bytes, mac_1_bytes, mac_2_bytes) = self.generate_ipv4_header_bytes(protocol_kind, &mut rng);
        }
        else {
            match rng.random_bool(0.5) {
                true => {
                    packet_bytes_size += GENERATED_IPV4_HEADER_LEN;
                    (ethertype_header_bytes, mac_1_bytes, mac_2_bytes) = self.generate_ipv4_header_bytes(protocol_kind, &mut rng);
                }
                false => {
                    packet_bytes_size += GENERATED_IPV6_HEADER_LEN;
                    (ethertype_header_bytes, mac_1_bytes, mac_2_bytes) = self.generate_ipv6_header_bytes(protocol_kind, &mut rng);
                }
            }
        }

        let mut bytes = Vec::with_capacity(packet_bytes_size);
        bytes.extend(mac_1_bytes);
        bytes.extend(mac_2_bytes);
        bytes.extend(ethertype_header_bytes);
        bytes.extend(protocol_bytes);
        bytes
    }

    fn generate_tcp_header_bytes(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(GENERATED_TCP_HEADER_LEN);
        for _ in 0..12 {
            bytes.push(rng.random_range(0..=255)); // ports, seq & ack num
        }

        bytes.push(0b_0101_0000); // data offset & reserved

        for _ in 0..7 {
            bytes.push(rng.random_range(0..=255)); // ports, seq & ack num
        }

        bytes
    }

    fn generate_udp_header_bytes(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(GENERATED_UDP_HEADER_LEN);
        for _ in 0..4 {
            bytes.push(rng.random_range(0..=255)); // ports
        }
        
        let len_bytes = (GENERATED_UDP_HEADER_LEN as u16).to_be_bytes();
        bytes.push(len_bytes[0]);
        bytes.push(len_bytes[1]);
        bytes.push(rng.random_range(0..=255)); // checksum
        bytes.push(rng.random_range(0..=255)); // checksum
        bytes
    }

    fn generate_ipv4_header_bytes(&self, protocol_kind: ProtocolKind, rng: &mut ThreadRng) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut bytes = Vec::with_capacity(GENERATED_IPV4_HEADER_LEN);
        bytes.push(0x08);   // ipv4 type
        bytes.push(0x00);   // ipv4 type
        bytes.push(0b_01000101);    // version - 4, ihl - 5
        bytes.push(rng.random_range(0..=255)); // dscp & ecn
        let total_len_bytes = (GENERATED_IPV4_HEADER_LEN as u16 + match protocol_kind {
            ProtocolKind::Tcp => GENERATED_TCP_HEADER_LEN as u16,
            ProtocolKind::Udp => GENERATED_UDP_HEADER_LEN as u16
        }).to_be_bytes(); // total len
        bytes.push(total_len_bytes[0]);
        bytes.push(total_len_bytes[1]);
        for _ in 0..5 {
            bytes.push(rng.random_range(0..=255)); // identification, flags, fragment_offset, time_to_live
        }

        bytes.push(match protocol_kind {
            ProtocolKind::Tcp => 6u8,
            ProtocolKind::Udp => 17u8
        }); // protocol

        bytes.push(rng.random_range(0..=255)); // checksum
        bytes.push(rng.random_range(0..=255)); // checksum
        let host_1 = self.ipv4_hosts.choose(rng).expect("There should be at least one ipv4 host");
        let host_2 = self.ipv4_hosts.choose(rng).expect("There should be at least one ipv4 host");
        bytes.extend(host_1.get_ip_bytes());
        bytes.extend(host_2.get_ip_bytes());
        let mac_1 = host_1.get_random_mac(rng);
        let mac_2 = host_2.get_random_mac(rng);
        (bytes, mac_1, mac_2)
    }

    fn generate_ipv6_header_bytes(&self, protocol_kind: ProtocolKind, rng: &mut ThreadRng) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut bytes = Vec::with_capacity(GENERATED_IPV6_HEADER_LEN);
        bytes.push(0x86);   // ipv6 type
        bytes.push(0xDD);   // ipv6 type
        bytes.push((0b_0110_0000 | rng.random_range(0..=15)) as u8);    // version - 4, dscp p1
        bytes.push(rng.random_range(0..=255)); // dscp & ecn
        bytes.push(rng.random_range(0..=255)); // flow_label
        bytes.push(rng.random_range(0..=255)); // flow_label
        let payload_len_bytes = match protocol_kind {
            ProtocolKind::Tcp => (GENERATED_TCP_HEADER_LEN as u16).to_be_bytes(),
            ProtocolKind::Udp => (GENERATED_UDP_HEADER_LEN as u16).to_be_bytes()
        };

        bytes.push(payload_len_bytes[0]); // payload_len
        bytes.push(payload_len_bytes[1]); // payload_len
        bytes.push(match protocol_kind {
            ProtocolKind::Tcp => 6,
            ProtocolKind::Udp => 17
        }); // next header

         bytes.push(rng.random_range(0..=255)); // hop limit

        let host_1 = self.ipv6_hosts.choose(rng).expect("There should be at least one ipv6 host");
        let host_2 = self.ipv6_hosts.choose(rng).expect("There should be at least one ipv6 host");
        bytes.extend(host_1.get_ip_bytes());
        bytes.extend(host_2.get_ip_bytes());
        let mac_1 = host_1.get_random_mac(rng);
        let mac_2 = host_2.get_random_mac(rng);
        (bytes, mac_1, mac_2)
    }

}

impl PacketSource for PacketFactory {
    fn run(self: Box<Self>, id: u8, packet_queue: Arc<dyn PacketQueue>, running: Arc<AtomicBool>, is_finished_sender: Sender<u8>) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let mut rng = rand::rng();
            let mut packets_created = 0usize;
            log!("PacketGenerator", log::Level::Debug, format!("[ID:{}] Started work. Packet limit: {}, interval: {} us, ipv4 addresses: {}, ipv6 addresses: {}", id, self.packets_limit, self.creation_interval.as_micros(), self.ipv4_hosts.len(), self.ipv6_hosts.len()));

            let mut next_tick = Instant::now();
            while running.load(Ordering::Relaxed) {
                match  Packet::create(self.generate_packet_bytes(&mut rng)) {
                    Ok(packet) => {
                        packets_created += 1;
                        packet_queue.push(Box::new(packet));
                    }
                    Err(e) => {
                        log!("PacketGenerator", log::Level::Error, format!("[ID:{}] Could not create packet: {:?}", id, e));
                    }
                }

                if self.packets_limit != 0 && packets_created >= self.packets_limit { break; }
                next_tick += self.creation_interval;
                if let Some(remaining) = next_tick.checked_duration_since(Instant::now()) {
                    std::thread::sleep(remaining);
                }
            }

            log!("PacketGenerator", log::Level::Debug, format!("[ID:{}] finished work. Created packets count: {}", id, packets_created));
            if let Err(e) = is_finished_sender.send(id) {
                panic!("Packet generator with ID: {} could not inform about finished work. Error: {}", id, e);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::test_init::get_logger;
    use crate::packet_queue::dummy_queue::DummyQueue;

    fn generate_some_random_packets(factory: PacketFactory, ipv4_addresses_count: &mut usize, ipv6_addresses_count: &mut usize, mac_addresses_count: &mut usize, had_ipv4_tcp: &mut bool, had_ipv4_udp: &mut bool, had_ipv6_tcp: &mut bool, had_ipv6_udp: &mut bool) {
        use crate::packet::{EtherType, Protocol};
        use std::collections::HashSet;
        use std::net::{Ipv4Addr, Ipv6Addr};

        let mut ipv4_addresses: HashSet<Ipv4Addr> = HashSet::new();
        let mut ipv6_addresses: HashSet<Ipv6Addr> = HashSet::new();
        let mut mac_addresses: HashSet<[u8; 6]> = HashSet::new();
        let mut rng = rand::rng();

        for _ in 0..1000 {
            let packet_creation_result = Packet::create(factory.generate_packet_bytes(&mut rng));
            assert!(packet_creation_result.is_ok());
            let packet = packet_creation_result.unwrap();
            mac_addresses.insert(packet.get_source_mac().clone());
            mac_addresses.insert(packet.get_destination_mac().clone());
            assert_eq!(packet.get_payload().len(), 0);
            match packet.get_ethertype() {
                EtherType::Ipv4(hdr) => {
                    ipv4_addresses.insert(hdr.get_source_address().clone());
                    ipv4_addresses.insert(hdr.get_destination_address().clone());

                    match packet.get_protocol() {
                        Some(Protocol::Tcp(_)) => {
                            *had_ipv4_tcp = true;
                        }
                        Some(Protocol::Udp(_)) => {
                            *had_ipv4_udp = true;
                        }
                        None => {
                            panic!("There should be a protocol in ipv4!");
                        }
                    }
                }
                EtherType::Ipv6(hdr) => {
                    ipv6_addresses.insert(hdr.get_source_address().clone());
                    ipv6_addresses.insert(hdr.get_destination_address().clone());

                    match packet.get_protocol() {
                        Some(Protocol::Tcp(_)) => {
                            *had_ipv6_tcp = true;
                        }
                        Some(Protocol::Udp(_)) => {
                            *had_ipv6_udp = true;
                        }
                        None => {
                            panic!("There should be a protocol in ipv6!");
                        }
                    }
                }
            }
        }

        *ipv4_addresses_count = ipv4_addresses.len();
        *ipv6_addresses_count = ipv6_addresses.len();
        *mac_addresses_count = mac_addresses.len();
    }

    #[test]
    fn generate_some_ipv4_random_packets() {
        let factory_creation_result = PacketFactory::create(10, 10, 10, 0, 1);
        assert!(factory_creation_result.is_ok());

        let mut had_ipv4_tcp = false;
        let mut had_ipv4_udp = false;
        let mut had_ipv6_tcp = false;
        let mut had_ipv6_udp = false;
        let mut ipv4_addresses_count = 0;
        let mut ipv6_addresses_count = 0;
        let mut mac_addresses_count = 0;
        generate_some_random_packets(factory_creation_result.unwrap(), &mut ipv4_addresses_count, &mut ipv6_addresses_count, &mut mac_addresses_count, &mut had_ipv4_tcp, &mut had_ipv4_udp, &mut had_ipv6_tcp, &mut had_ipv6_udp);

        assert!(had_ipv4_tcp);
        assert!(had_ipv4_udp);
        assert!(!had_ipv6_tcp);
        assert!(!had_ipv6_udp);
        assert_eq!(ipv4_addresses_count, 10);
        assert_eq!(ipv6_addresses_count, 0);
        assert_eq!(mac_addresses_count, 10);
    }

    #[test]
    fn generate_some_ipv6_random_packets() {
        let factory_creation_result = PacketFactory::create(10, 10, 0, 10, 3);
        assert!(factory_creation_result.is_ok());

        let mut had_ipv4_tcp = false;
        let mut had_ipv4_udp = false;
        let mut had_ipv6_tcp = false;
        let mut had_ipv6_udp = false;
        let mut ipv4_addresses_count = 0;
        let mut ipv6_addresses_count = 0;
        let mut mac_addresses_count = 0;
        generate_some_random_packets(factory_creation_result.unwrap(), &mut ipv4_addresses_count, &mut ipv6_addresses_count, &mut mac_addresses_count, &mut had_ipv4_tcp, &mut had_ipv4_udp, &mut had_ipv6_tcp, &mut had_ipv6_udp);

        assert!(!had_ipv4_tcp);
        assert!(!had_ipv4_udp);
        assert!(had_ipv6_tcp);
        assert!(had_ipv6_udp);
        assert_eq!(ipv4_addresses_count, 0);
        assert_eq!(ipv6_addresses_count, 10);
        assert_eq!(mac_addresses_count, 30);
    }

    #[test]
    fn generate_some_ipv6_and_ipv4_random_packets() {
        let factory_creation_result = PacketFactory::create(10, 10, 20, 20, 5);
        assert!(factory_creation_result.is_ok());

        let mut had_ipv4_tcp = false;
        let mut had_ipv4_udp = false;
        let mut had_ipv6_tcp = false;
        let mut had_ipv6_udp = false;
        let mut ipv4_addresses_count = 0;
        let mut ipv6_addresses_count = 0;
        let mut mac_addresses_count = 0;
        generate_some_random_packets(factory_creation_result.unwrap(), &mut ipv4_addresses_count, &mut ipv6_addresses_count, &mut mac_addresses_count, &mut had_ipv4_tcp, &mut had_ipv4_udp, &mut had_ipv6_tcp, &mut had_ipv6_udp);

        assert!(had_ipv4_tcp);
        assert!(had_ipv4_udp);
        assert!(had_ipv6_tcp);
        assert!(had_ipv6_udp);
        assert_eq!(ipv4_addresses_count, 20);
        assert_eq!(ipv6_addresses_count, 20);
        assert_eq!(mac_addresses_count, 200);
    }

    #[test]
    fn generate_some_ipv4_random_packets_with_only_one_address_available() {
        let factory_creation_result = PacketFactory::create(10, 10, 1, 0, 1);
        assert!(factory_creation_result.is_ok());

        let mut had_ipv4_tcp = false;
        let mut had_ipv4_udp = false;
        let mut had_ipv6_tcp = false;
        let mut had_ipv6_udp = false;
        let mut ipv4_addresses_count = 0;
        let mut ipv6_addresses_count = 0;
        let mut mac_addresses_count = 0;
        generate_some_random_packets(factory_creation_result.unwrap(), &mut ipv4_addresses_count, &mut ipv6_addresses_count, &mut mac_addresses_count, &mut had_ipv4_tcp, &mut had_ipv4_udp, &mut had_ipv6_tcp, &mut had_ipv6_udp);

        assert!(had_ipv4_tcp);
        assert!(had_ipv4_udp);
        assert!(!had_ipv6_tcp);
        assert!(!had_ipv6_udp);
        assert_eq!(ipv4_addresses_count, 1);
        assert_eq!(ipv6_addresses_count, 0);
        assert_eq!(mac_addresses_count, 1);
    }

    #[test]
    fn generate_some_ipv6_random_packets_with_only_one_address_available() {
        let factory_creation_result = PacketFactory::create(10, 10, 0, 1, 1);
        assert!(factory_creation_result.is_ok());

        let mut had_ipv4_tcp = false;
        let mut had_ipv4_udp = false;
        let mut had_ipv6_tcp = false;
        let mut had_ipv6_udp = false;
        let mut ipv4_addresses_count = 0;
        let mut ipv6_addresses_count = 0;
        let mut mac_addresses_count = 0;
        generate_some_random_packets(factory_creation_result.unwrap(), &mut ipv4_addresses_count, &mut ipv6_addresses_count, &mut mac_addresses_count, &mut had_ipv4_tcp, &mut had_ipv4_udp, &mut had_ipv6_tcp, &mut had_ipv6_udp);

        assert!(!had_ipv4_tcp);
        assert!(!had_ipv4_udp);
        assert!(had_ipv6_tcp);
        assert!(had_ipv6_udp);
        assert_eq!(ipv4_addresses_count, 0);
        assert_eq!(ipv6_addresses_count, 1);
        assert_eq!(mac_addresses_count, 1);
    }

    #[test]
    fn cannot_create_factory() {
        assert!(PacketFactory::create(10001, 20, 20, 20, 20).is_err());
        assert!(PacketFactory::create(0, 20, 20, 20, 20).is_err());
        assert!(PacketFactory::create(2, 20, 0, 0, 20).is_err());
        assert!(PacketFactory::create(2, 20, 0, 5, 0).is_err());
        assert!(PacketFactory::create(2, 20, 5, 0, 0).is_err());
    }

    #[test]
    fn run_with_unlimited_packets() {
        let (tx, _rx) = std::sync::mpsc::channel::<u8>();
        let _ = get_logger();
        let dummy_queue: Arc<dyn PacketQueue> = Arc::new(DummyQueue::create());
        let factory: Box<dyn PacketSource> = Box::new(PacketFactory::create(100, 0, 100, 100, 10).expect("Factory should be created for test"));
        let running_flag = Arc::new(AtomicBool::new(true));
        let joiner = factory.run(4, Arc::clone(&dummy_queue), Arc::clone(&running_flag), tx);
        std::thread::sleep(Duration::from_secs(2));
        running_flag.store(false, Ordering::Relaxed);
        let _ = joiner.join();
        let created_packets = dummy_queue.get_queued_packets_count();
        assert!(created_packets >= 190 && created_packets <= 210);
    }

    #[test]
    fn run_with_limited_packets() {
        let (tx, _rx) = std::sync::mpsc::channel::<u8>();
        let _ = get_logger();
        let dummy_queue: Arc<dyn PacketQueue> = Arc::new(DummyQueue::create());
        let factory: Box<dyn PacketSource> = Box::new(PacketFactory::create(10000, 25000, 100, 100, 10).expect("Factory should be created for test"));
        let running_flag = Arc::new(AtomicBool::new(true));
        let start = Instant::now();
        let joiner = factory.run(3, Arc::clone(&dummy_queue), Arc::clone(&running_flag), tx);
        let _ = joiner.join();
        assert!(start.elapsed().as_millis() < 2600);
        let created_packets = dummy_queue.get_queued_packets_count();
        assert_eq!(created_packets, 25000);
    }
}