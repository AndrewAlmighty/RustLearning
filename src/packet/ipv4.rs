use crate::packet::{PacketError, ProtocolKind, Ecn};

use std::net::Ipv4Addr;

#[derive(Debug, PartialEq)]
pub struct IPv4Header {
    ihl: u8,
    dscp: u8,
    ecn: Ecn,
    total_length: u16,
    identification: u16,
    flags: u8,
    fragment_offset: u16,
    time_to_live: u8,
    header_checksum: u16,
    src_address: Ipv4Addr,
    dst_address: Ipv4Addr,
    options: Vec<u8>
}

impl IPv4Header {
    pub fn create(bytes: &[u8]) -> Result<(Self, ProtocolKind), PacketError> {
        let bytes_len = bytes.len();
        if bytes_len == 0 {
            return Err(PacketError::Ipv4TooShort(bytes_len));
        }

        let version: u8 = (bytes[0] & 0b_11110000) >> 4;
        if version != 4 {
            return Err(PacketError::Ipv4WrongVersion(version));
        }

        const MAX_IPV4_HEADER_LEN: u8 = 15;
        const MIN_IPV4_HEADER_LEN: u8 = 5;

        let ihl: u8 = bytes[0] & 0b_00001111;
        if ihl < MIN_IPV4_HEADER_LEN || ihl > MAX_IPV4_HEADER_LEN {
            return Err(PacketError::Ipv4WrongIhl(ihl));
        }

        let header_len = (ihl as usize) * 4;
        if header_len > bytes_len {
            return Err(PacketError::Ipv4TooShort(bytes_len));
        }

        let dscp = (bytes[1] & 0b_11111100) >> 2;
        let ecn = match bytes[1] & 0b_00000011 {
            0b_00 => Ecn::NonEct,
            0b_01 => Ecn::Ect0,
            0b_10 => Ecn::Ect1,
            0b_11 => Ecn::Ce,
            rest => { return Err(PacketError::Ipv4EcnWrong(rest)); }
        };

        let total_length = (bytes[2] as u16) << 8 | bytes[3] as u16;
        let identification = (bytes[4] as u16) << 8 | bytes[5] as u16;
        let flags = (bytes[6] & 0b_11100000) >> 5;
        let fragment_offset = ((bytes[6] & 0b_00011111) as u16) << 8 | bytes[7] as u16;
        let time_to_live = bytes[8];
        let protocol = match bytes[9] {
            6 => ProtocolKind::Tcp,
            17 => ProtocolKind::Udp,
            x => { return Err(PacketError::Ipv4UnknownProtocol(x)); }
        };
        let header_checksum = (bytes[10] as u16) << 8 | bytes[11] as u16;
        let source_address = Ipv4Addr::new(bytes[12], bytes[13], bytes[14], bytes[15]);
        let destination_address = Ipv4Addr::new(bytes[16], bytes[17], bytes[18], bytes[19]);

        let options: Vec<u8> =
        if ihl <= 5 {
            Vec::new()
        }
        else {
            bytes[20..header_len].to_vec()
        };

        Ok(( IPv4Header {
            ihl: ihl,
            dscp: dscp,
            ecn: ecn,
            total_length: total_length,
            identification: identification,
            flags: flags,
            fragment_offset: fragment_offset,
            time_to_live: time_to_live,
            header_checksum: header_checksum,
            src_address: source_address,
            dst_address: destination_address,
            options: options
            },
            protocol
        ))
    }

    pub fn get_length(&self) -> usize {
        (self.ihl as usize) * 4
    }

    pub fn get_dscp(&self) -> u8 {
        self.dscp
    }

    pub fn get_ecn(&self) -> Ecn {
        self.ecn
    }

    pub fn get_total_length(&self) -> u16 {
        self.total_length
    }

    pub fn get_identification(&self) -> u16 {
        self.identification
    }

    pub fn get_flags(&self) -> u8 {
        self.flags
    }

    pub fn get_fragment_offset(&self) -> u16 {
        self.fragment_offset
    }

    pub fn get_time_to_live(&self) -> u8 {
        self.time_to_live
    }

    pub fn get_header_checksum(&self) -> u16 {
        self.header_checksum
    }

    pub fn get_source_address(&self) -> &Ipv4Addr {
        &self.src_address
    }

    pub fn get_destination_address(&self) -> &Ipv4Addr {
        &self.dst_address
    }

    pub fn get_options(&self) -> &Vec<u8> {
        &self.options
    }
}

#[test]
fn test_ipv4_1() {
    let ipv4_creation_result = IPv4Header::create(&[0x45, 0x00, 0x00, 0x28, 0x12, 0x34, 0x40, 0x00, 0x40, 0x06, 0xab, 0xcd, 192, 168, 1, 1, 192, 168, 1, 2]);
    assert!(ipv4_creation_result.is_ok());
    let (ipv4_header, protocol) = ipv4_creation_result.unwrap();
    assert_eq!(protocol, ProtocolKind::Tcp);
    assert_eq!(ipv4_header.ihl, 5);
    assert_eq!(ipv4_header.dscp, 0);
    assert_eq!(ipv4_header.ecn, Ecn::NonEct);
    assert_eq!(ipv4_header.total_length, 40);
    assert_eq!(ipv4_header.identification, 0x1234);
    assert_eq!(ipv4_header.flags, 2); 
    assert_eq!(ipv4_header.fragment_offset, 0);
    assert_eq!(ipv4_header.time_to_live, 64);
    assert_eq!(ipv4_header.header_checksum, 0xabcd);
    assert_eq!(ipv4_header.src_address, Ipv4Addr::new(192, 168, 1, 1));
    assert_eq!(ipv4_header.dst_address, Ipv4Addr::new(192, 168, 1, 2));
    assert!(ipv4_header.options.is_empty());
}


#[test]
fn test_ipv4_2() {
    let ipv4_creation_result = IPv4Header::create(&[0x46, 0x04, 0x00, 0x30, 0xab, 0xcd, 0x00, 0x00, 0x20, 0x11, 0xde, 0xad, 10, 0, 0, 1, 10, 0, 0, 2, 0xde, 0xad, 0xbe, 0xef]);
    assert!(ipv4_creation_result.is_ok());
    let (ipv4_header, protocol) = ipv4_creation_result.unwrap();
    assert_eq!(protocol, ProtocolKind::Udp);
    assert_eq!(ipv4_header.ihl, 6);
    assert_eq!(ipv4_header.dscp, 1);
    assert_eq!(ipv4_header.ecn, Ecn::NonEct);
    assert_eq!(ipv4_header.total_length, 48);
    assert_eq!(ipv4_header.identification, 0xabcd);
    assert_eq!(ipv4_header.flags, 0);
    assert_eq!(ipv4_header.fragment_offset, 0);
    assert_eq!(ipv4_header.time_to_live, 32);
    assert_eq!(ipv4_header.header_checksum, 0xdead);
    assert_eq!(ipv4_header.src_address, Ipv4Addr::new(10, 0, 0, 1));
    assert_eq!(ipv4_header.dst_address, Ipv4Addr::new(10, 0, 0, 2));
    assert_eq!(ipv4_header.options, vec![0xde, 0xad, 0xbe, 0xef]);
}

#[test]
fn cannot_create_ipv4() {
    let mut bytes = [0u8; 20];

    bytes[0] = 0x45;
    bytes[9] = 99;
    assert_eq!(IPv4Header::create(&bytes), Err(PacketError::Ipv4UnknownProtocol(99)));

    bytes[9] = 0;
    bytes[0] = 0x65;
    assert_eq!(IPv4Header::create(&bytes), Err(PacketError::Ipv4WrongVersion(6)));

    bytes[0] = 0x44;
    assert_eq!(IPv4Header::create(&bytes), Err(PacketError::Ipv4WrongIhl(4)));
}