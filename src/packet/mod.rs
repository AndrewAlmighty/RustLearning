pub mod tcp;
pub mod udp;
pub mod ipv4;
pub mod ipv6;

use tcp::TcpHeader;
use udp::UdpHeader;
use ipv4::IPv4Header;
use ipv6::IPv6Header;

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

#[derive(Debug, PartialEq)]
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
    src_mac: [u8; 6],
    dst_mac:  [u8; 6],
    ethertype: EtherType,
    protocol: Option<Protocol>,
    payload: Vec<u8>
}

impl Packet {
    pub fn create(bytes: Vec<u8>) -> Result<Self, PacketError> {
        const MIN_ETHERNET_FRAME_LEN: usize = 14;

        let bytes_len = bytes.len();
        if bytes_len < MIN_ETHERNET_FRAME_LEN {
            return Err(PacketError::EthFrameTooShort(bytes_len));
        }

        let dst_mac: [u8; 6] = bytes[0..6].try_into().expect("DST_MAC - FRAME LEN CHECKED AND FAILED");
        let src_mac: [u8; 6] = bytes[6..12].try_into().expect("SRC_MAC - FRAME LEN CHECKED AND FAILED");

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
                current_idx += UdpHeader::get_length();
                Some(Protocol::Udp(hdr))
            }
            None => None
        };

        Ok(Packet {
            src_mac: src_mac,
            dst_mac: dst_mac,
            ethertype: ethertype,
            protocol: protocol,
            payload: bytes[current_idx..].to_vec()
        })
    }
}

#[test]
fn test_packet_1() {
    let packet_creation_result = Packet::create(vec![0xac, 0x12, 0x03, 0x16, 0x53, 0x6e, 0xf4, 0xc1, 0x14, 0x84, 0xe7, 0x36, 0x08, 0x00, 0x45, 0x00, 0x00, 0x40, 0x10, 0x3b, 0x00, 0x00, 0x78, 0x06, 0xb4, 0xb4, 0xac, 0xd9, 0x10, 0x36, 0xc0, 0xa8, 0x00, 0x11, 0x01, 0xbb, 0x8e, 0x84, 0x97, 0x69, 0x98, 0xb4, 0xdd, 0x9c, 0x0e, 0x13, 0xb0, 0x10, 0x04, 0x0c, 0x60, 0xaf, 0x00, 0x00, 0x01, 0x01, 0x08, 0x0a, 0x36, 0x0a, 0xb8, 0x55, 0x8e, 0x26, 0x5e, 0x55, 0x01, 0x01, 0x05, 0x0a, 0xdd, 0x9c, 0x0d, 0xec, 0xdd, 0x9c, 0x0e, 0x13]);
    assert!(packet_creation_result.is_ok());
}