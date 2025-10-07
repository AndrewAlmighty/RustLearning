pub mod tcp;
pub mod udp;
pub mod ipv4;
pub mod ipv6;

use tcp::TcpHeader;
use udp::UdpHeader;
use ipv4::IPv4Header;
use ipv6::IPv6Header;

use chrono::{DateTime, Utc};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::net::IpAddr;

pub type MacAddress = [u8; 6];

#[derive(Debug, PartialEq, Clone)]
pub enum PacketError {
    EthFrameTooShort(usize),
    UnsupportedEtherType(u16),
    CannotCreateMac(bool, String), // bool - is_dst

    Ipv4WrongVersion(u8),
    Ipv4TooShort(usize),
    Ipv4WrongIhl(u8),
    Ipv4EcnWrong(u8),
    Ipv4UnknownProtocol(u8),

    Ipv6WrongVersion(u8),
    Ipv6TooShort(usize),
    Ipv6EcnWrong(u8),
    Ipv6UnsupportedExtension(u8),

    TcpTooShort(usize),
    TcpReservedIsNonZero(u8),
    TcpDataOffsetWrong(u8),
    TcpParseOptionsTooShort(usize, usize),
    
    UdpTooShort(usize),
    UpdLengthWrong(u16),
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Ecn {
    NonEct,
    Ect0,
    Ect1,
    Ce
}

#[derive(Debug, PartialEq)]
pub enum ProtocolKind {
    Tcp,
    Udp
}

pub enum Protocol {
    Tcp(TcpHeader),
    Udp(UdpHeader)
}

pub enum EtherType {
    Ipv4(IPv4Header),
    Ipv6(IPv6Header)
}

pub struct Packet {
    id: usize,
    timestamp: DateTime<Utc>,
    src_mac: MacAddress,
    dst_mac:  MacAddress,
    ethertype: EtherType,
    protocol: Option<Protocol>,
    payload: Vec<u8>
}

impl Packet {
    pub fn create(bytes: Vec<u8>) -> Result<Self, PacketError> {
        const MIN_ETHERNET_FRAME_LEN: usize = 14;
        static PACKET_COUNTER: AtomicUsize = AtomicUsize::new(0);

        let bytes_len = bytes.len();
        if bytes_len < MIN_ETHERNET_FRAME_LEN {
            return Err(PacketError::EthFrameTooShort(bytes_len));
        }

        let dst_mac: MacAddress = bytes[0..6].try_into().expect("DST_MAC - FRAME LEN CHECKED AND FAILED");
        let src_mac: MacAddress = bytes[6..12].try_into().expect("SRC_MAC - FRAME LEN CHECKED AND FAILED");

        let mut current_idx = MIN_ETHERNET_FRAME_LEN;

        let (ethertype, protocol_kind) = match (bytes[12] as u16) << 8 | bytes[13] as u16 {
            0x0800 => {
                let (ipv4_hdr, protocol_kind) = IPv4Header::create(&bytes[14..])?;
                current_idx += ipv4_hdr.get_length();
                (EtherType::Ipv4(ipv4_hdr), Some(protocol_kind))
            }
            0x86DD => {
                let (ipv6_hdr, protocol_kind) = IPv6Header::create(&bytes[14..])?;
                current_idx += ipv6_hdr.get_length();
                (EtherType::Ipv6(ipv6_hdr), Some(protocol_kind))
            }
            v => {
                return Err(PacketError::UnsupportedEtherType(v));
            }
        };

        let protocol = match protocol_kind {
            Some(ProtocolKind::Tcp) => {
                let hdr = TcpHeader::create(&bytes[current_idx..])?;
                current_idx += hdr.get_length();
                Some(Protocol::Tcp(hdr))
            }
            Some(ProtocolKind::Udp) => {
                let hdr = UdpHeader::create(&bytes[current_idx..])?;
                current_idx += UdpHeader::get_udp_length();
                Some(Protocol::Udp(hdr))
            }
            None => None
        };

        let payload = 
            if current_idx < bytes_len { bytes[current_idx..].to_vec() }
            else if current_idx == bytes_len { Vec::new() }
            else { return Err(PacketError::EthFrameTooShort(bytes_len)); };

        let id = PACKET_COUNTER.fetch_add(1, Ordering::Relaxed);

        Ok(Packet {
            id: id,
            timestamp: Utc::now(),
            src_mac: src_mac,
            dst_mac: dst_mac,
            ethertype: ethertype,
            protocol: protocol,
            payload: payload
        })
    }

    pub fn get_id(&self) -> usize {
        self.id
    }

    pub fn get_timestamp(&self) -> &DateTime<Utc> {
        &self.timestamp
    }

    pub fn get_source_mac(&self) -> &MacAddress {
        &self.src_mac
    }

    pub fn get_destination_mac(&self) -> &MacAddress {
        &self.dst_mac
    }

    pub fn get_source_address(&self) -> IpAddr {
        match &self.ethertype{
            EtherType::Ipv4(hdr) => {
                IpAddr::V4(hdr.get_source_address().clone())
            }
            EtherType::Ipv6(hdr) => {
                IpAddr::V6(hdr.get_source_address().clone())
            }
        }
    }

    pub fn get_destination_address(&self) -> IpAddr {
        match &self.ethertype{
            EtherType::Ipv4(hdr) => {
                IpAddr::V4(hdr.get_destination_address().clone())
            }
            EtherType::Ipv6(hdr) => {
                IpAddr::V6(hdr.get_destination_address().clone())
            }
        }
    }

    pub fn get_ethertype(&self) -> &EtherType {
        &self.ethertype
    }

    pub fn get_protocol(&self) -> &Option<Protocol> {
        &self.protocol
    }

    pub fn get_payload(&self) -> &Vec<u8> {
        &self.payload
    }
}

#[test]
fn test_packet_1() {
    let packet_creation_result = Packet::create(vec![
        0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36, 0xac, 0x12, 0x03, 0x16, 0x53, 0x6e, 0x08, 0x00, 0x45, 0x00,
        0x00, 0x34, 0x1b, 0xf9, 0x40, 0x00, 0x40, 0x06, 0x6b, 0x1e, 0xc0, 0xa8, 0x00, 0x11, 0x22, 0x78,
        0xd0, 0x7b, 0xda, 0x7e, 0x01, 0xbb, 0xbb, 0x54, 0xfb, 0x91, 0xa3, 0xac, 0x8a, 0x49, 0x80, 0x10,
        0x01, 0xc2, 0xb3, 0xd3, 0x00, 0x00, 0x01, 0x01, 0x08, 0x0a, 0x57, 0x6b, 0x1d, 0x54, 0x9b, 0x1d,
        0xf1, 0xaa
    ]);

    assert!(packet_creation_result.is_ok());

    let packet = packet_creation_result.unwrap();
    assert_eq!(packet.src_mac, [0xac, 0x12, 0x03, 0x16, 0x53, 0x6e]);
    assert_eq!(packet.dst_mac, [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36]);
    assert!(matches!(packet.ethertype, EtherType::Ipv4(_)));

    if let EtherType::Ipv4(hdr) = packet.ethertype {
        assert_eq!(hdr.get_length(), 20);
        assert_eq!(hdr.get_dscp(), 0);
        assert_eq!(hdr.get_ecn(), Ecn::NonEct);
        assert_eq!(hdr.get_total_length(), 52);
        assert_eq!(hdr.get_identification(), 7161);
        assert_eq!(hdr.get_flags(), 0x2);
        assert_eq!(hdr.get_fragment_offset(), 0);
        assert_eq!(hdr.get_time_to_live(), 64);
        assert_eq!(hdr.get_header_checksum(), 0x6b1e);
        assert_eq!(*hdr.get_source_address(), std::net::Ipv4Addr::new(192, 168, 0 ,17));
        assert_eq!(*hdr.get_destination_address(), std::net::Ipv4Addr::new(34, 120, 208 ,123));
        assert_eq!(hdr.get_options().len(), 0);
    }
    else {
        panic!("expected IPv4 header");
    }

    assert!(matches!(packet.protocol, Some(Protocol::Tcp(_))));
    if let Some(Protocol::Tcp(hdr)) = packet.protocol {
        assert_eq!(hdr.get_length(), 32);
        assert_eq!(hdr.get_destination_port(), 443);
        assert_eq!(hdr.get_source_port(), 55934);
        assert_eq!(hdr.get_sequence_number(), 3142908817);
        assert_eq!(hdr.get_acknowledgment_number(), 2745993801);
        assert_eq!(hdr.get_nonce_sum(), false);
        assert_eq!(hdr.get_window(), 450);
        assert_eq!(hdr.get_checksum(), 0xb3d3);
        assert_eq!(hdr.get_urgent_ptr(), None);
        use crate::packet::tcp::Flag;
        assert_eq!(hdr.get_flags(), vec![Flag::Ack]);
        let tcp_options = hdr.get_options();
        assert_eq!(tcp_options.len(), 1);
        assert_eq!(tcp_options[0].get_kind(), 8);
        assert_eq!(*tcp_options[0].get_value(), vec![0x57, 0x6b, 0x1d, 0x54, 0x9b, 0x1d, 0xf1, 0xaa]);
    }
    else {
        panic!("expected Tcp header");
    }

    assert_eq!(packet.payload.len(), 0);
}


#[test]
fn test_packet_2() {
    let packet_creation_result = Packet::create(vec![
        0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36, 0xac, 0x12, 0x03, 0x16, 0x53, 0x6e, 0x86, 0xdd, 0x60, 0x06,
        0xe1, 0x79, 0x00, 0x40, 0x06, 0x40, 0x2a, 0x02, 0x2a, 0x40, 0x58, 0xf2, 0xb7, 0x00, 0xfd, 0xe5,
        0xdb, 0x28, 0x19, 0x94, 0x84, 0x45, 0x2a, 0x03, 0x28, 0x80, 0xf3, 0x2e, 0x00, 0x90, 0xfa, 0xce,
        0xb0, 0x0c, 0x00, 0x00, 0x00, 0x02, 0xc2, 0x42, 0x01, 0xbb, 0x7a, 0xf0, 0x3e, 0x21, 0x9a, 0x74,
        0x55, 0xdb, 0x80, 0x18, 0x03, 0x84, 0xcc, 0x83, 0x00, 0x00, 0x01, 0x01, 0x08, 0x0a, 0x0e, 0x06,
        0xa7, 0xf3, 0xb5, 0x74, 0x83, 0x00, 0x17, 0x03, 0x03, 0x00, 0x1b, 0xd1, 0xc4, 0x95, 0x0a, 0x32,
        0xd6, 0xae, 0x08, 0x75, 0xcd, 0x35, 0x6b, 0xb6, 0x25, 0x9d, 0xfb, 0x82, 0x2f, 0x00, 0x65, 0xfb,
        0xd5, 0x2d, 0xbf, 0x13, 0x83, 0x70
    ]);
    assert!(packet_creation_result.is_ok());
    let packet = packet_creation_result.unwrap();
    assert_eq!(packet.src_mac, [0xac, 0x12, 0x03, 0x16, 0x53, 0x6e]);
    assert_eq!(packet.dst_mac, [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36]);

    assert!(matches!(packet.ethertype, EtherType::Ipv6(_)));
    if let EtherType::Ipv6(hdr) = packet.ethertype {
        assert_eq!(hdr.get_length(), 40);
        assert_eq!(hdr.get_dscp(), 0);
        assert_eq!(hdr.get_ecn(), Ecn::NonEct);
        assert_eq!(hdr.get_flow_label(), 0x06e179);
        assert_eq!(hdr.get_payload_length(), 64);
        assert_eq!(hdr.get_hop_limit(), 64);
        assert_eq!(*hdr.get_source_address(), std::net::Ipv6Addr::new(0x2a02, 0x2a40, 0x58f2, 0xb700, 0xfde5, 0xdb28, 0x1994, 0x8445));
        assert_eq!(*hdr.get_destination_address(), std::net::Ipv6Addr::new(0x2a03, 0x2880, 0xf32e, 0x90, 0xface, 0xb00c, 0x0, 0x2));
        assert_eq!(hdr.get_extension_headers().len(), 0); 

    }
    else {
        panic!("expected IPv6 header");
    }

    assert!(matches!(packet.protocol, Some(Protocol::Tcp(_))));
    if let Some(Protocol::Tcp(hdr)) = packet.protocol {
        assert_eq!(hdr.get_length(), 32);
        assert_eq!(hdr.get_destination_port(), 443);
        assert_eq!(hdr.get_source_port(), 49730);
        assert_eq!(hdr.get_sequence_number(), 2062564897);
        assert_eq!(hdr.get_acknowledgment_number(), 2591315419);
        assert_eq!(hdr.get_nonce_sum(), false);
        assert_eq!(hdr.get_window(), 900);
        assert_eq!(hdr.get_checksum(), 0xcc83);
        assert_eq!(hdr.get_urgent_ptr(), None);
        use crate::packet::tcp::Flag;
        assert_eq!(hdr.get_flags(), vec![Flag::Ack, Flag::Psh]);
        let tcp_options = hdr.get_options();
        assert_eq!(tcp_options.len(), 1);
        assert_eq!(tcp_options[0].get_kind(), 8);
        assert_eq!(*tcp_options[0].get_value(), vec![0x0e, 0x06, 0xa7, 0xf3, 0xb5, 0x74, 0x83, 0x00]);
    }
    else {
        panic!("expected Tcp header");
    }

    assert_eq!(packet.payload.len(), 32);
}

#[test]
fn test_packet_3() {
    let packet_creation_result = Packet::create(vec![
        0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36, 0xac, 0x12, 0x03, 0x16, 0x53, 0x6e, 0x86, 0xdd, 0x6b, 0x84,
        0x33, 0xe3, 0x00, 0x38, 0x11, 0x40, 0x2a, 0x02, 0x2a, 0x40, 0x58, 0xf2, 0xb7, 0x00, 0xfd, 0xe5,
        0xdb, 0x28, 0x19, 0x94, 0x84, 0x45, 0x26, 0x20, 0x00, 0x2d, 0x40, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0xb1, 0xc1, 0x00, 0x7b, 0x00, 0x38, 0x41, 0xf5, 0x23, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7d, 0xe9, 0x26, 0xde, 0x4d, 0x91, 0x47, 0x34
    ]);
    assert!(packet_creation_result.is_ok());
    let packet = packet_creation_result.unwrap();
    assert_eq!(packet.src_mac, [0xac, 0x12, 0x03, 0x16, 0x53, 0x6e]);
    assert_eq!(packet.dst_mac, [0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36]);

    assert!(matches!(packet.ethertype, EtherType::Ipv6(_)));
    if let EtherType::Ipv6(hdr) = packet.ethertype {
        assert_eq!(hdr.get_length(), 40);
        assert_eq!(hdr.get_dscp(), 46);
        assert_eq!(hdr.get_ecn(), Ecn::NonEct);
        assert_eq!(hdr.get_flow_label(), 0x0433e3);
        assert_eq!(hdr.get_payload_length(), 56);
        assert_eq!(hdr.get_hop_limit(), 64);
        assert_eq!(*hdr.get_source_address(), std::net::Ipv6Addr::new(0x2a02, 0x2a40, 0x58f2, 0xb700, 0xfde5, 0xdb28, 0x1994, 0x8445));
        assert_eq!(*hdr.get_destination_address(), std::net::Ipv6Addr::new(0x2620, 0x002d, 0x4000, 0x1, 0x0, 0x0, 0x0, 0x40));
        assert_eq!(hdr.get_extension_headers().len(), 0); 

    }
    else {
        panic!("expected IPv6 header");
    }

    assert!(matches!(packet.protocol, Some(Protocol::Udp(_))));
    if let Some(Protocol::Udp(hdr)) = packet.protocol {
        assert_eq!(hdr.get_source_port(), 45505);
        assert_eq!(hdr.get_destination_port(), 123);
        assert_eq!(hdr.get_length(), 56);
        assert_eq!(hdr.get_checksum(), 0x41f5);
    }
    else {
        panic!("expected Udp header");
    }

    assert_eq!(packet.payload.len(), 48);
}

#[test]
fn test_packet_4() {
    let packet_creation_result = Packet::create(vec![
        0x01, 0x00, 0x5e, 0x00, 0x00, 0xfb, 0xac, 0x12, 0x03, 0x16, 0x53, 0x6e, 0x08, 0x00, 0x45, 0x00,
        0x00, 0x49, 0xaf, 0x78, 0x40, 0x00, 0xff, 0x11, 0x2a, 0x76, 0xc0, 0xa8, 0x00, 0x11, 0xe0, 0x00,
        0x00, 0xfb, 0x14, 0xe9, 0x14, 0xe9, 0x00, 0x35, 0xa1, 0xfb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x5f, 0x69, 0x70, 0x70, 0x04, 0x5f, 0x74, 0x63, 0x70,
        0x05, 0x6c, 0x6f, 0x63, 0x61, 0x6c, 0x00, 0x00, 0x0c, 0x00, 0x01, 0x05, 0x5f, 0x69, 0x70, 0x70,
        0x73, 0xc0, 0x11, 0x00, 0x0c, 0x00, 0x01
    ]);

    assert!(packet_creation_result.is_ok());
    let packet = packet_creation_result.unwrap();
    assert_eq!(packet.src_mac, [0xac, 0x12, 0x03, 0x16, 0x53, 0x6e]);
    assert_eq!(packet.dst_mac, [0x01, 0x00, 0x5e, 0x00, 0x00, 0xfb]);

    assert!(matches!(packet.ethertype, EtherType::Ipv4(_)));
    if let EtherType::Ipv4(hdr) = packet.ethertype {
        assert_eq!(hdr.get_length(), 20);
        assert_eq!(hdr.get_dscp(), 0);
        assert_eq!(hdr.get_ecn(), Ecn::NonEct);
        assert_eq!(hdr.get_total_length(), 73);
        assert_eq!(hdr.get_identification(), 44920);
        assert_eq!(hdr.get_flags(), 0x2);
        assert_eq!(hdr.get_fragment_offset(), 0);
        assert_eq!(hdr.get_time_to_live(), 255);
        assert_eq!(hdr.get_header_checksum(), 0x2a76);
        assert_eq!(*hdr.get_source_address(), std::net::Ipv4Addr::new(192, 168, 0 ,17));
        assert_eq!(*hdr.get_destination_address(), std::net::Ipv4Addr::new(224, 0, 0, 251));
        assert_eq!(hdr.get_options().len(), 0);
    }
    else {
        panic!("expected IPv4 header");
    }

    assert!(matches!(packet.protocol, Some(Protocol::Udp(_))));
    if let Some(Protocol::Udp(hdr)) = packet.protocol {
        assert_eq!(hdr.get_source_port(), 5353);
        assert_eq!(hdr.get_destination_port(), 5353);
        assert_eq!(hdr.get_length(), 53);
        assert_eq!(hdr.get_checksum(), 0xa1fb);
    }
    else {
        panic!("expected Udp header");
    }

    assert_eq!(packet.payload.len(), 45);
}