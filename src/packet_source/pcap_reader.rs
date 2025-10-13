use crate::packet_source::PacketSource;
use crate::packet_queue::PacketQueue;
use crate::packet::Packet;
use crate::log::*;


use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::{Instant, Duration};

pub struct PcapReader {
    pcap: File,
    packet_creation_interval: Duration,
    loop_reading: bool,
    packet_record_header_buf: [u8; 16]
}

impl PcapReader {
    pub fn create(pcap: PathBuf, packets_per_second: u64, loop_reading: bool) -> Result<Self, String> {
        const PACKETS_PER_SECONDS_MAX_LIMIT: u64 = 10000;

        if packets_per_second > PACKETS_PER_SECONDS_MAX_LIMIT {
            return Err(format!("Cannot create PcapReader - requested to create {} packets per second, which is above limit: {}", packets_per_second, PACKETS_PER_SECONDS_MAX_LIMIT));
        }

        if packets_per_second == 0 {
            return Err("Cannot create PcapReader - packets per second must be bigger than 0".to_string());
        }

        if !pcap.exists() {
            return Err(format!("Cannot create PcapReader - file {} does not exists", pcap.display()));
        }

        if !pcap.is_file() {
            return Err(format!("Cannot create PcapReader - expected file, {} is not a file", pcap.display()));
        }

        if let Some(ext) = pcap.extension() {
            if ext != "pcap" {
                return Err(format!("Cannot create PcapReader - expected file with .pcap extension, file {} does not have such extension", pcap.display()));
            }
        }
        else {
            return Err(format!("Cannot create PcapReader - expected file with .pcap extension, file {} does not have such extension", pcap.display()));
        }

        let mut file = match File::open(&pcap) {
            Ok(f) => f,
            Err(e) => {
                return Err(format!("Cannot create PcapReader - could not open file: {}, error: {}", pcap.display(), e));
            }
        };

        if let Err(e) = Self::verify_header(&mut file) {
            return Err(format!("Cannot create PcapReader - verification of pcap file: {} header failed - error: {}", pcap.display(), e));
        }

        const MICROSECONDS_IN_SECOND: u64 = 1_000_000;
        let packet_creation_interval = MICROSECONDS_IN_SECOND / packets_per_second;

        Ok(PcapReader {pcap: file, packet_creation_interval: Duration::from_micros(packet_creation_interval), loop_reading: loop_reading, packet_record_header_buf: [0u8; 16]} )
    }

    fn verify_header(file: &mut File) -> Result<(), String> {
        let mut buf = vec![0u8; 4];
        let mut read_4_bytes = |b: &mut Vec<u8>| -> Result<(), String> {
            match file.read(b) {
                Ok(4) => { return Ok(()); },
                Ok(n) => {
                    return Err(format!("Could not read 4 bytes of pcap header, Read {} bytes", n));
                }
                Err(e) => {
                    return Err(format!("Could not read 4 bytes of pcap header, Err: {}", e));
                }
            }
        };

        read_4_bytes(&mut buf)?;
        if buf != [0xD4, 0xC3, 0xB2, 0xA1] && buf != [0x4D, 0x3C, 0xB2, 0xA1] {
            return Err(format!("Magic number is not right: {:?}", buf));
        }

        read_4_bytes(&mut buf)?;
        const PCAP_MAJOR_VERSION_REQUIRED: u16 = 2;
        let major_version = (buf[1] as u16) << 8 | buf[0] as u16;

        if major_version != PCAP_MAJOR_VERSION_REQUIRED {
            return Err(format!("Pcap header has different major version: {}, we support: {}", major_version, PCAP_MAJOR_VERSION_REQUIRED));
        }

        read_4_bytes(&mut buf)?; // skip reserved1 field
        read_4_bytes(&mut buf)?; // skip reserved2 field
        read_4_bytes(&mut buf)?;
        let snap_len = (buf[3] as u32) << 24 | (buf[2] as u32) << 16 | (buf[1] as u32) << 8 | buf[0] as u32;
        if snap_len == 0 {
            return Err(format!("Pcap header's snaplen has value: {}, should has 0", snap_len));
        }

        read_4_bytes(&mut buf)?;
        if buf[3] & 0b_00001111 != 0 {
            return Err("Pcap header's should has bits set to 0 after FCS".to_string());
        }

        if buf[2] != 0 {
            return Err("Pcap header's should has bits set to 0 before LinkType".to_string());
        }

        Ok(())
    }

    fn get_packet_length(&mut self) -> Option<u32> {
        match self.pcap.read(&mut self.packet_record_header_buf) {
            Ok(16) => Some((self.packet_record_header_buf[11] as u32) << 24 | (self.packet_record_header_buf[10] as u32) << 16 | (self.packet_record_header_buf[9] as u32) << 8 | self.packet_record_header_buf[8] as u32),
            Ok(0) => None,
            Ok(n) => {
                log!("PcapReader", log::Level::Error, format!("Read wrong number of bytes when reading packet record header: {}. Getting packet size failed.", n));
                None
            }
            Err(e) => {
                log!("PcapReader", log::Level::Error, format!("Error during pcap read: {}.", e));
                None
            }
        }
    }

    fn read_packet_bytes(&mut self) -> Vec<u8> {
        const FIRST_PACKET_RECORD_POS: u64 = 24;
        let mut packet_size = self.get_packet_length();
        if packet_size.is_none() {
            if !self.loop_reading { return Vec::new(); }
            if let Err(e) = self.pcap.seek(SeekFrom::Start(FIRST_PACKET_RECORD_POS)) {
                log!("PcapReader", log::Level::Error, format!("Error during pcap seek to beggining: {}.", e));
                return Vec::new();
            }
            packet_size = self.get_packet_length();
            if packet_size.is_none() {
                return Vec::new();
            }
        }

        let bytes_to_read = packet_size.unwrap() as usize;
        let mut buf = vec![0u8; bytes_to_read];
        match self.pcap.read(&mut buf) {
            Ok(n) if n == bytes_to_read => buf,
            Ok(0) => Vec::new(),
            Ok(n) => {
                log!("PcapReader", log::Level::Error, format!("Expected to read: {} bytes of packet, but read {} bytes.", bytes_to_read, n));
                Vec::new()
            }
            Err(e) => {
                log!("PcapReader", log::Level::Error, format!("Error during pcap packet read: {}.", e));
                Vec::new()
            }
        }
        
    }
}

impl PacketSource for PcapReader {
    fn run(mut self: Box<Self>, id: u8, packet_queue: Arc<dyn PacketQueue>, running: Arc<AtomicBool>, is_finished_sender: Sender<u8>) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let mut packets_created = 0usize;
            log!("PcapReader", log::Level::Debug, format!("[ID:{}] Started work. Interval: {} us, loop_reading: {}", id, self.packet_creation_interval.as_micros(), self.loop_reading));

            let mut next_tick = Instant::now();
            while running.load(Ordering::Relaxed) {
                let packet_bytes = self.read_packet_bytes();
                if packet_bytes.is_empty() {
                    break;
                }

                match Packet::create(packet_bytes) {
                    Ok(packet) => {
                        packets_created += 1;
                        packet_queue.push(Box::new(packet));
                        next_tick += self.packet_creation_interval;
                        if let Some(remaining) = next_tick.checked_duration_since(Instant::now()) {
                            std::thread::sleep(remaining);
                        }
                    }
                    Err(e) => {
                        log!("PcapReader", log::Level::Error, format!("[ID:{}] Could not create packet: {:?}", id, e));
                    }
                }
            }

            log!("PcapReader", log::Level::Debug, format!("[ID:{}] Finished work. Created packets count: {}", id, packets_created));
            if let Err(e) = is_finished_sender.send(id) {
                panic!("Pcap reader with ID: {} could not inform about finished work. Error: {}", id, e);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::test_init::get_logger;
    use crate::packet_queue::dummy_queue::DummyQueue;

    #[test]
    fn test_reading_pcap_file_no_loop_read() {
        let (tx, _rx) = std::sync::mpsc::channel::<u8>();
        let _ = get_logger();
        let dummy_queue: Arc<dyn PacketQueue> = Arc::new(DummyQueue::create());
        let pcap_reader: Box<dyn PacketSource> = Box::new(PcapReader::create(PathBuf::from("basic_packet_test.pcap"), 1, false).expect("Pcap reader should be created for test"));
        let running_flag = Arc::new(AtomicBool::new(true));
        let start = Instant::now();
        let joiner = pcap_reader.run(1, Arc::clone(&dummy_queue), Arc::clone(&running_flag), tx);
        let _ = joiner.join();
        assert!(start.elapsed().as_millis() < 4100);
        assert_eq!(dummy_queue.get_queued_packets_count(), 4);
    }

    #[test]
    fn test_reading_pcap_file_loop_read() {
        let _ = get_logger();
        let dummy_queue: Arc<dyn PacketQueue> = Arc::new(DummyQueue::create());
        let pcap_reader: Box<dyn PacketSource> = Box::new(PcapReader::create(PathBuf::from("basic_packet_test.pcap"), 10, true).expect("Pcap reader should be created for test"));
        let running_flag = Arc::new(AtomicBool::new(true));
        let (tx, _rx) = std::sync::mpsc::channel::<u8>();
        let joiner = pcap_reader.run(2, Arc::clone(&dummy_queue), Arc::clone(&running_flag), tx);
        std::thread::sleep(Duration::from_millis(2001));
        running_flag.store(false, Ordering::Relaxed);
        let _ = joiner.join();
        let created_packets = dummy_queue.get_queued_packets_count();
        assert!(created_packets > 20 && created_packets < 22);
    }
}
