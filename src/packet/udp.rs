use crate::packet::PacketError;

const UDP_HEADER_LEN: usize = 8;

#[derive(Debug, PartialEq)]
pub struct UdpHeader {
    src_port: u16,
    dst_port: u16,
    length: u16,
    checksum: u16
}

impl UdpHeader {
    pub fn create(bytes: &[u8]) -> Result<Self, PacketError> {
        let bytes_len = bytes.len();
        if bytes_len < UDP_HEADER_LEN {
            return Err(PacketError::UdpTooShort(bytes_len));
        }

        let src_port = (bytes[0] as u16) << 8 | bytes[1] as u16;
        let dst_port = (bytes[2] as u16) << 8 | bytes[3] as u16;
        let length = (bytes[4] as u16) << 8 | bytes[5] as u16;
        let checksum = (bytes[6] as u16) << 8 | bytes[7] as u16;

        if length < UDP_HEADER_LEN as u16 {
            return Err(PacketError::UpdLengthWrong(length));
        }

        if length as usize > bytes.len() {
            return Err(PacketError::UdpTooShort(bytes_len));
        }

        Ok(UdpHeader {
            src_port: src_port,
            dst_port: dst_port,
            length: length,
            checksum: checksum
        })
    }

    pub fn get_udp_length() -> usize {
        UDP_HEADER_LEN
    }

    pub fn get_source_port(&self) -> u16 {
        self.src_port
    }

    pub fn get_destination_port(&self) -> u16 {
        self.dst_port
    }

    pub fn get_length(&self) -> u16 {
        self.length
    }

    pub fn get_checksum(&self) -> u16 {
        self.checksum
    }
}


#[test]
fn test_udp() {
    let udp_creation_result = UdpHeader::create(&[0x00, 0x35, 0x30, 0x39, 0x00, 0x08, 0x1A, 0x2B]);
    assert!(udp_creation_result.is_ok());
    let udp = udp_creation_result.unwrap();
    assert_eq!(udp.src_port, 53);
    assert_eq!(udp.dst_port, 12345);
    assert_eq!(udp.length, 8);
    assert_eq!(udp.checksum, 0x1A2B);
}

#[test]
fn cannot_create_udp() {
    assert_eq!(UdpHeader::create(&[0x00, 0x50, 0x01, 0xBB, 0x00, 0x08]),  Err(PacketError::UdpTooShort(6)));
    assert_eq!(UdpHeader::create(&[0x12, 0x34, 0x56, 0x78, 0x00, 0x06, 0x00, 0x00]),  Err(PacketError::UpdLengthWrong(6)));
    assert_eq!(UdpHeader::create(&[0x00, 0x35, 0x30, 0x39, 0x00, 0x14, 0x1A, 0x2B, 0x01, 0x02, 0x03, 0x04]),  Err(PacketError::UdpTooShort(12)));
}