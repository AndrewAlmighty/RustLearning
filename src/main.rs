pub mod log;
pub mod packet;
mod packet_source;
mod packet_queue;
mod packet_processor;
mod host_cache;

use crate::log::*;
use crate::packet_source::PacketSource;
use crate::packet_source::packet_factory::PacketFactory;
use crate::packet_source::pcap_reader::PcapReader;
use crate::packet_queue::PacketQueue;
use crate::packet_queue::ring_buffer::RingBuffer;
use crate::packet_processor::PacketProcessor;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

use serde::Deserialize;

#[derive(clap::Parser)]
pub struct Config {
    #[arg(long, default_value = "info", help = "Logging level. Available levels: debug, info, error.")]
    log_level: String,
    #[arg(long, help = "Path to log file")]
    log_file: Option<PathBuf>,
    #[arg(long, help = "Size of the packet queue (must be power of two)")]
    queue_size: u32,
    #[arg(long, help = "Number of packet processors which will handle packets from queue")]
    packet_processors: u8,
    #[arg(long, required = true, help = "\
        Packet sources described in json. You can provide multiple packet sources, each will run in different thread. Types:\n\
        - factory - will create packets with random bytes, with specific settings. Options:\n\
            * packets_per_second,\n\
            * packets_limit (will stop after create n packets, 0 is unlimited),\n\
            * ipv4_hosts (number of used ipv4 ips),\n\
            * ipv6_hosts (number of used ipv6 ips),\n\
            * macs_per_host (number of mac addresses used for each ip).\n\
        - pcap_reader - will read pcap file and create packets. Options:\n\
            * pcap (path fo pcap file),\n\
            * packets_per_second,\n\
            * loop_reading (after reading whole file reader will move to beggining and start again).\n\
        Example json:\n\
        {\"factory\": [{\"packets_per_second\": 4, \"packets_limit\": 0, \"ipv4_hosts\": 20, \"ipv6_hosts\": 10, \"macs_per_host\": 1}],\n\
        \"pcap_reader\": [{\"pcap\": \"example.pcap\", \"packets_per_second\": 10000, \"loop_reading\": true},\n\
        {\"pcap\": \"another.pcap\", \"packets_per_second\": 10, \"loop_reading\": false}]}
    ")]
    packet_sources: String
}

fn parse_config(config: Config) -> Result<(Logger, Vec<Box<dyn PacketSource>>, Arc<dyn PacketQueue>, Vec<PacketProcessor>), String> {
    let logger = match Logger::create(config.log_file, config.log_level) {
        Ok(l) => l,
        Err(e) => { return Err(format!("Error when creating logger: {}", e)); }
    };

    let packet_sources_json = match serde_json::from_str(config.packet_sources.as_str()) {
        Ok(v) => {
            if let serde_json::Value::Object(obj) = v {
                obj
            }
            else {
                return Err("packet_sources json should be an object!".to_string());
            }
        }
        Err(e) => { return Err(format!("Error during parsing packet_sources json: {}", e)); }
    };

    if packet_sources_json.len() > 100 {
        return Err("Cannot have more that 100 packet sources".to_string());
    }

    let mut packet_sources = Vec::<Box<dyn PacketSource>>::new();

    #[derive(Deserialize)]
    struct FactoryOpts {
        packets_per_second: u64,
        packets_limit: usize,
        ipv4_hosts: usize,
        ipv6_hosts: usize,
        macs_per_host: usize
    }

    #[derive(Deserialize)]
    struct PcapReaderOpts {
        pcap: PathBuf,
        packets_per_second: u64,
        loop_reading: bool
    }

    for (k, v) in packet_sources_json {
        match k.as_str() {
            "factory" => {
                let factories = match serde_json::from_value::<Vec<FactoryOpts>>(v) {
                    Ok(f) => f,
                    Err(e) => { return Err(format!("Error during parsing factories: {}", e)); }
                };

                for opts in factories {
                    match PacketFactory::create(opts.packets_per_second, opts.packets_limit, opts.ipv4_hosts, opts.ipv6_hosts, opts.macs_per_host) {
                        Ok(factory) => { packet_sources.push(Box::new(factory)); }
                        Err(e) => { return Err(e); }
                    }
                }
            }
            "pcap_reader" => {
                let pcap_readers = match serde_json::from_value::<Vec<PcapReaderOpts>>(v) {
                    Ok(r) => r,
                    Err(e) => { return Err(format!("Error during parsing pcap readers: {}", e)); }
                };

                for opts in pcap_readers {
                    match PcapReader::create(opts.pcap, opts.packets_per_second, opts.loop_reading) {
                        Ok(reader) => { packet_sources.push(Box::new(reader)); }
                        Err(e) => { return Err(e); }
                    }
                }
            }
            unrecognized => {
                return Err(format!("Unregonized packet source: {}", unrecognized));
            }
        }
    }

    if packet_sources.is_empty() {
        return Err("At least one packet source is required".to_string());
    }

    let packet_queue: Arc<dyn PacketQueue> = match RingBuffer::create(config.queue_size) {
        Ok(ring_buffer) => Arc::new(ring_buffer),
        Err(e) => { return Err(e); }
    };

    let packet_processors = (0..config.packet_processors).map(|i| PacketProcessor::create(i)).collect::<Vec<_>>();

    Ok((logger, packet_sources, packet_queue, packet_processors))
}

fn wait_for_processes_end(mut join_handlers: HashMap<u8, JoinHandle<()>>, is_finished_receiver: Receiver<u8>, handle_packet_sources: bool) {
    while !join_handlers.is_empty() {
        match is_finished_receiver.recv() {
            Ok(id) => {
                let join_handle = 
                if handle_packet_sources {
                    log!("App", log::Level::Debug, format!("Removed thread for packet source with id: {}.", id));
                    join_handlers.remove(&id).expect(format!("There is no entry for packet source with id: {}", id).as_str())
                }
                else {
                    log!("App", log::Level::Debug, format!("Removed thread for packet processor with id: {}.", id));
                    join_handlers.remove(&id).expect(format!("There is no entry for packet processor with id: {}", id).as_str())
                };

                let _ = join_handle.join();
            }
            Err(e) => {
                panic!("Is finished receiver got error: {}", e);
            }
        }
    }
}

fn main() {
    let config = <Config as clap::Parser>::parse();
    let (logger, packet_sources, packet_queue, packet_processors) = match parse_config(config) {
        Err(e) => {
            println!("{}", e);
            return;
        }

        Ok((logger, packet_sources, packet_queue, packet_processors)) => (logger, packet_sources, packet_queue, packet_processors),
    };

    let packet_sources_running_flag = Arc::new(AtomicBool::new(true));
    let r_flag = packet_sources_running_flag.clone();
    ctrlc::set_handler(move || {
        r_flag.store(false, Ordering::Relaxed);
    }).expect("Error setting Ctrl-C handler");

    let logger_is_packet_sources_running_flag = logger.run();
    log!("App", log::Level::Info, "Starting work. Send SIGINT to stop.".to_string());

    let (packet_processor_is_finished_sender, packet_processor_is_finished_receiver) = std::sync::mpsc::channel::<u8>();
    let packet_processors_running_flag = Arc::new(AtomicBool::new(true));
    let mut packet_processors_join_handlers = HashMap::with_capacity(packet_processors.len());

    for packet_processor in packet_processors {
        let id = packet_processor.get_id();
        packet_processors_join_handlers.insert(id, packet_processor.run(Arc::clone(&packet_queue), Arc::clone(&packet_processors_running_flag), packet_processor_is_finished_sender.clone()));
    }

    drop(packet_processor_is_finished_sender);

    let (packet_source_is_finished_sender, packet_source_is_finished_receiver) = std::sync::mpsc::channel::<u8>();
    let mut packet_sources_join_handlers = HashMap::with_capacity(packet_sources.len());
    
    for (i, packet_source) in packet_sources.into_iter().enumerate() {
        packet_sources_join_handlers.insert(i as u8, packet_source.run(i as u8, Arc::clone(&packet_queue), Arc::clone(&packet_sources_running_flag), packet_source_is_finished_sender.clone()));
    }

    drop(packet_source_is_finished_sender);
    wait_for_processes_end(packet_sources_join_handlers, packet_source_is_finished_receiver, true);
    packet_processors_running_flag.store(false, Ordering::Relaxed);
    wait_for_processes_end(packet_processors_join_handlers, packet_processor_is_finished_receiver, false);

    log!("App", log::Level::Info, format!("Packet queue accepted {} packets.", packet_queue.get_queued_packets_count()));
    log!("App", log::Level::Info, format!("Packet queue dropped {} packets.", packet_queue.get_dropped_packets_count()));
    log!("App", log::Level::Info, "Finished work.".to_string());
    std::thread::sleep(std::time::Duration::from_millis(1));
    logger_is_packet_sources_running_flag.store(false, Ordering::Relaxed);
}
