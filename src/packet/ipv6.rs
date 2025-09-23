use crate::packet::{PacketError, ProtocolKind, Ecn};

use std::net::Ipv6Addr;

const MIN_IPV6_HEADER_LEN: usize = 40;

#[derive(PartialEq, Debug)]
enum ExtensionHeader {
    HopByHopOpts(Vec<u8>),
    Routing(Vec<u8>),
    Fragment(Vec<u8>),
    AuthenticationHeader(Vec<u8>),
    DestinationOptions(Vec<u8>)
}

#[derive(Debug, PartialEq)]
pub struct IPv6Header {
    dscp: u8,
    ecn: Ecn,
    flow_label: u32,
    payload_length: u16,
    hop_limit: u8,
    src_addr: Ipv6Addr,
    dst_addr: Ipv6Addr,
    extension_headers: Vec<ExtensionHeader>
}

impl IPv6Header {
    pub fn create(bytes: &[u8]) -> Result<(Self, ProtocolKind), PacketError> {
        let bytes_len = bytes.len();
        if bytes_len < MIN_IPV6_HEADER_LEN {
            return Err(PacketError::Ipv6TooShort(bytes_len));
        }

        let version: u8 = (bytes[0] & 0b_11110000) >> 4;
        if version != 6 {
            return Err(PacketError::Ipv6WrongVersion(version));
        }

        let dscp = ((bytes[0] & 0b_00001111) << 2) | ((bytes[1] & 0b_11000000) >> 6);
        let ecn = match (bytes[1] & 0b_00110000) >> 4 {
            0b_00 => Ecn::NonEct,
            0b_01 => Ecn::Ect0,
            0b_10 => Ecn::Ect1,
            0b_11 => Ecn::Ce,
            rest => { return Err(PacketError::Ipv6EcnWrong(rest)); }
        };

        let flow_label = ((bytes[1] as u32 & 0x0F) << 16) | (bytes[2] as u32) << 8 | bytes[3] as u32;
        let payload_length = (bytes[4] as u16) << 8 | bytes[5] as u16;

        let hop_limit = bytes[7];
        let src_addr = Ipv6Addr::new(
            (bytes[8] as u16) << 8 | bytes[9] as u16,
            (bytes[10] as u16) << 8 | bytes[11] as u16,
            (bytes[12] as u16) << 8 | bytes[13] as u16,
            (bytes[14] as u16) << 8 | bytes[15] as u16,
            (bytes[16] as u16) << 8 | bytes[17] as u16,
            (bytes[18] as u16) << 8 | bytes[19] as u16,
            (bytes[20] as u16) << 8 | bytes[21] as u16,
            (bytes[22] as u16) << 8 | bytes[23] as u16
        );
        let dst_addr = Ipv6Addr::new(
            (bytes[24] as u16) << 8 | bytes[25] as u16,
            (bytes[26] as u16) << 8 | bytes[27] as u16,
            (bytes[28] as u16) << 8 | bytes[29] as u16,
            (bytes[30] as u16) << 8 | bytes[31] as u16,
            (bytes[32] as u16) << 8 | bytes[33] as u16,
            (bytes[34] as u16) << 8 | bytes[35] as u16,
            (bytes[36] as u16) << 8 | bytes[37] as u16,
            (bytes[38] as u16) << 8 | bytes[39] as u16
        );

        let (protocol, extension_headers) = Self::parse_extension_headers(&bytes, bytes[6])?;
        Ok((IPv6Header{
                dscp: dscp,
                ecn: ecn,
                flow_label: flow_label,
                payload_length: payload_length,
                hop_limit: hop_limit,
                src_addr: src_addr,
                dst_addr: dst_addr,
                extension_headers: extension_headers
            },
            protocol))
    }

    fn parse_extension_headers(bytes: &[u8], mut next_header_val: u8) -> Result<(ProtocolKind, Vec<ExtensionHeader>), PacketError> {
        let mut extension_headers: Vec<ExtensionHeader> = Vec::new();
        let mut next_header_begin_idx = 40usize;
        let bytes_len = bytes.len();

        let mut parse_extension = |extension_header_len: usize, header_begin_idx: usize, make_variant: fn(Vec<u8>) -> ExtensionHeader| -> Result<(u8, usize), PacketError> {
            let next_header_begin_idx = header_begin_idx + extension_header_len;
            if header_begin_idx >= bytes_len || next_header_begin_idx > bytes_len {
                return Err(PacketError::Ipv6TooShort(bytes_len));
            }

            let next_header = bytes[header_begin_idx];
            let ext_bytes = bytes[header_begin_idx..next_header_begin_idx].to_vec();
            extension_headers.push(make_variant(ext_bytes));
            Ok((next_header, next_header_begin_idx))
        };

        let protocol: ProtocolKind;

        loop {
            println!("next_header_val: {}", next_header_val);
            let header_begin_at = next_header_begin_idx;
            match next_header_val {
                0 => { (next_header_val, next_header_begin_idx) = parse_extension(8 + ((bytes[header_begin_at + 1] as usize) * 8), header_begin_at, ExtensionHeader::HopByHopOpts)?; }
                6 => { protocol = ProtocolKind::Tcp; break; }
                17 => { protocol = ProtocolKind::Udp; break; }
                43 => { (next_header_val, next_header_begin_idx) = parse_extension(8, header_begin_at, ExtensionHeader::Fragment)?; }
                44 => { (next_header_val, next_header_begin_idx) = parse_extension(8, header_begin_at, ExtensionHeader::Routing)?; }
                51 => { (next_header_val, next_header_begin_idx) = parse_extension(((bytes[header_begin_at + 1] as usize) + 2) * 4, header_begin_at, ExtensionHeader::AuthenticationHeader)?; }
                60 => { (next_header_val, next_header_begin_idx) = parse_extension(((bytes[header_begin_at + 1] as usize) + 2) * 4, header_begin_at, ExtensionHeader::DestinationOptions)?; }
                n => { return Err(PacketError::Ipv6UnsupportedExtension(n)); }
            }
        }
        Ok((protocol, extension_headers))
    }

    pub fn get_length(&self) -> usize {
        self.extension_headers.iter().map(|ext| match ext {
            ExtensionHeader::HopByHopOpts(v) |
            ExtensionHeader::Routing(v) |
            ExtensionHeader::Fragment(v) |
            ExtensionHeader::AuthenticationHeader(v) |
            ExtensionHeader::DestinationOptions(v) => v.len() }).sum::<usize>() + MIN_IPV6_HEADER_LEN
    }
}

#[test]
fn test_ipv6_1() {
    let ipv6_creation_result = IPv6Header::create(&[0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x40, 0x20,0x01,0x0d,0xb8,0x85,0xa3,0x00,0x00,0x00,0x00,0x8a,0x2e,0x03,0x70,0x73,0x34, 0x20,0x01,0x0d,0xb8,0x85,0xa3,0x00,0x00,0x00,0x00,0x8a,0x2e,0x03,0x70,0x73,0x35]);
    assert!(ipv6_creation_result.is_ok());
    let (ipv6_header, protocol) = ipv6_creation_result.unwrap();
    assert_eq!(protocol, ProtocolKind::Tcp);
    assert_eq!(ipv6_header.dscp, 0);
    assert_eq!(ipv6_header.ecn, Ecn::NonEct);
    assert_eq!(ipv6_header.flow_label, 0);
    assert_eq!(ipv6_header.payload_length, 0);
    assert_eq!(ipv6_header.hop_limit, 64);
    assert_eq!(ipv6_header.src_addr, Ipv6Addr::new(
        0x2001,0x0db8,0x85a3,0x0000,0x0000,0x8a2e,0x0370,0x7334
    ));
    assert_eq!(ipv6_header.dst_addr, Ipv6Addr::new(
        0x2001,0x0db8,0x85a3,0x0000,0x0000,0x8a2e,0x0370,0x7335
    ));
    assert_eq!(ipv6_header.extension_headers.len(), 0);
}

#[test]
fn test_ipv6_2() {
    let ipv6_creation_result = IPv6Header::create(&[0x6A,0x90,0xAB,0xCD,0x00,0x0A,0x00,0x40,0x20,0x01,0x0d,0xb8,0x85,0xa3,0x00,0x01,0x00,0x00,0x8a,0x2e,0x03,0x70,0x73,0x34,0x20,0x01,0x0d,0xb8,0x85,0xa3,0x00,0x02,0x00,0x00,0x8a,0x2e,0x03,0x70,0x73,0x35,6,0,1,2,3,4,5,6]);
    assert!(ipv6_creation_result.is_ok());
    let (ipv6_header, protocol) = ipv6_creation_result.unwrap();
    assert_eq!(protocol, ProtocolKind::Tcp);
    assert_eq!(ipv6_header.dscp, 42);
    assert_eq!(ipv6_header.ecn, Ecn::Ect0);
    assert_eq!(ipv6_header.flow_label, 0xABCD);
    assert_eq!(ipv6_header.payload_length, 10);
    assert_eq!(ipv6_header.hop_limit, 64);
    assert_eq!(ipv6_header.extension_headers.len(), 1);
    match &ipv6_header.extension_headers[0] {
        ExtensionHeader::HopByHopOpts(data) => assert_eq!(data.len(), 8),
        _ => panic!("Expected HopByHopOpts"),
    }
}

#[test]
fn test_ipv6_3() {
    let ipv6_creation_result = IPv6Header::create(&[0x6C,0xA0,0x12,0x34,0x00,0x0C,0x11,0x40,0x20,0x01,0x0d,0xb8,0x00,0x01,0x00,0x00,0x00,0x00,0x8a,0x2e,0x03,0x70,0x73,0x36,0x20,0x01,0x0d,0xb8,0x00,0x02,0x00,0x00,0x8a,0x2e,0x03,0x70,0x73,0x37, 0x00, 0x00]);
    assert!(ipv6_creation_result.is_ok());
    let (ipv6_header, protocol) = ipv6_creation_result.unwrap();
    assert_eq!(protocol, ProtocolKind::Udp);
    assert_eq!(ipv6_header.dscp, 50);
    assert_eq!(ipv6_header.ecn, Ecn::Ect1);
    assert_eq!(ipv6_header.flow_label, 0x1234);
    assert_eq!(ipv6_header.payload_length, 12);
    assert_eq!(ipv6_header.hop_limit, 64);
    assert_eq!(ipv6_header.extension_headers.len(), 0);
}

#[test]
fn test_ipv6_4() {
    let ipv6_creation_result = IPv6Header::create(&[
        // IPv6 header (40 bytes)
        0x6F, 0xF0, 0x12, 0x34, // version=6, dscp=63, ecn=0b11 (CE), flow_label=0x1234
        0x00, 0x28,             // payload length = 40
        0x00, 0x40,             // next header = 0 (Hop-by-Hop), hop limit = 64
        // src_addr
        0x20,0x01,0x0d,0xb8,0x00,0x01,0x00,0x00,0x00,0x00,0x8a,0x2e,0x03,0x70,0x73,0x34,
        // dst_addr
        0x20,0x01,0x0d,0xb8,0x00,0x02,0x00,0x00,0x00,0x00,0x8a,0x2e,0x03,0x70,0x73,0x35,
        // Hop-by-Hop Options (8 bytes)
        44, 0, 1,2,3,4,5,6,       // next header = 44 (Routing)
        // Routing Header (8 bytes)
        43, 0, 11,12,13,14,15,16, // next header = 51 (Authentication)
        // Fragment Header (8 bytes)
        51, 0, 21,22,23,24,25,26, // next header = 60 (Destination Options)
        // Authentication Header (12 bytes)
        60, 1, 31,32,33,34,35,36,37,38,39,40, // next header = 6 (TCP)
        // Destination Options (8 bytes)
        6, 0, 41,42,43,44,45,46  // next header = 6 (TCP)
    ]);

    assert!(ipv6_creation_result.is_ok());
    let (ipv6_header, protocol) = ipv6_creation_result.unwrap();
    assert_eq!(protocol, ProtocolKind::Tcp);
    assert_eq!(ipv6_header.dscp, 63);
    assert_eq!(ipv6_header.ecn, Ecn::Ce);
    assert_eq!(ipv6_header.flow_label, 0x1234);
    assert_eq!(ipv6_header.payload_length, 40);
    assert_eq!(ipv6_header.hop_limit, 64);
    assert_eq!(ipv6_header.src_addr, Ipv6Addr::new(
        0x2001,0x0db8,0x0001,0x0000,0x0000,0x8a2e,0x0370,0x7334
    ));
    assert_eq!(ipv6_header.dst_addr, Ipv6Addr::new(
        0x2001,0x0db8,0x0002,0x0000,0x0000,0x8a2e,0x0370,0x7335
    ));
    assert_eq!(ipv6_header.extension_headers.len(), 5);
    match &ipv6_header.extension_headers[0] { ExtensionHeader::HopByHopOpts(_) => {}, _ => panic!("Expected HopByHopOpts") }
    match &ipv6_header.extension_headers[1] { ExtensionHeader::Routing(_) => {}, _ => panic!("Expected Routing") }
    match &ipv6_header.extension_headers[2] { ExtensionHeader::Fragment(_) => {}, _ => panic!("Expected Fragment") }
    match &ipv6_header.extension_headers[3] { ExtensionHeader::AuthenticationHeader(_) => {}, _ => panic!("Expected AuthenticationHeader") }
    match &ipv6_header.extension_headers[4] { ExtensionHeader::DestinationOptions(_) => {}, _ => panic!("Expected DestinationOptions") }
}

#[test]
fn cannot_create_ipv6() {
    // Case 1: Too short (less than 40 bytes)
    let bytes_too_short = [0u8; 30];
    assert_eq!(IPv6Header::create(&bytes_too_short), Err(PacketError::Ipv6TooShort(30)));

    // Case 2: Wrong version (not 6)
    let mut bytes_wrong_version = [0u8; 40];
    bytes_wrong_version[0] = 0x50; // version 5
    assert_eq!(IPv6Header::create(&bytes_wrong_version), Err(PacketError::Ipv6WrongVersion(5)));

    // Case 3: Unsupported extension header
    let mut bytes_unsupported_ext = [0u8; 48];
    // Minimal IPv6 header
    bytes_unsupported_ext[0] = 0x60; // version=6, DSCP=0, ECN=0
    bytes_unsupported_ext[6] = 99;   // next header = 99 (unsupported)
    assert_eq!(IPv6Header::create(&bytes_unsupported_ext), Err(PacketError::Ipv6UnsupportedExtension(99)));

    // Case 4: ECN wrong value (invalid bits, should never happen but test defensive path)
    let mut bytes_ecn_wrong = [0u8; 40];
    bytes_ecn_wrong[0] = 0x60; // version=6
    bytes_ecn_wrong[1] = 0x30; // ECN bits 0b1100, invalid pattern (masked in your code, will extract 0b11 >> 4 = 0b11? careful)
    // Actually, to trigger EcnWrong, we need a value outside 0..=3 in `(bytes[1] & 0b_00110000) >> 4`.
    // But bits 5-4 of bytes[1] are only 2 bits => max 0b11, so EcnWrong cannot happen normally.
    // We can simulate it by changing the match arm to return EcnWrong for testing:
    // For now, skip EcnWrong as impossible with your current mask.

    // Case 5: Too short while parsing extensions
    let mut bytes_ext_too_short = [0u8; 45];
    bytes_ext_too_short[0] = 0x60;
    bytes_ext_too_short[6] = 0; // next header = 0 (Hop-by-Hop)
    // The parser will try to read at least 8 bytes for Hop-by-Hop but we only have 5 bytes after header
    assert_eq!(IPv6Header::create(&bytes_ext_too_short), Err(PacketError::Ipv6TooShort(45)));
}