use crate::packet::PacketError;

const MINIMAL_TCP_HEADER_LEN: usize = 20;

#[derive(PartialEq, Debug)]
pub enum Flag {
    Cwr,
    Ece,
    Urg,
    Ack,
    Psh,
    Rst,
    Syn,
    Fin
}

#[derive(Debug, PartialEq)]
pub struct TcpOption {
    kind: u8,
    value: Vec<u8>
}

impl TcpOption {
    pub fn get_kind(&self) -> u8 { self.kind }
    pub fn get_value(&self) -> &Vec<u8> { &self.value }
}

#[derive(Debug, PartialEq)]
pub struct TcpHeader {
    src_port: u16,
    dst_port: u16,
    seq_num: u32,
    ack_num: u32,
    data_offset: u8,
    nonce_sum: bool,
    flags: u8,
    window: u16,
    checksum: u16,
    urgent_ptr: Option<u16>,
    options: Vec<TcpOption>
}

impl TcpHeader {
    pub fn create(bytes: &[u8]) -> Result<Self, PacketError> {
        let bytes_len = bytes.len();
        if bytes_len < MINIMAL_TCP_HEADER_LEN {
            return Err(PacketError::TcpTooShort(bytes_len));
        }

        let reserved = (bytes[12] & 0b_00001110) >> 1;
        if reserved != 0 {
            return Err(PacketError::TcpReservedIsNonZero(reserved));
        }

        let data_offset = (bytes[12] & 0b_11110000) >> 4;
        if data_offset < 5 {
            return Err(PacketError::TcpDataOffsetWrong(data_offset));
        }

        let options_len = ((data_offset - 5) * 4) as usize;
        let header_with_options_len = MINIMAL_TCP_HEADER_LEN + options_len;
        if bytes_len < header_with_options_len {
            return Err(PacketError::TcpTooShort(bytes_len));
        }

        let src_port = (bytes[0] as u16) << 8 | (bytes[1] as u16);
        let dst_port = (bytes[2] as u16) << 8 | (bytes[3] as u16);
        let seq_num = (bytes[4] as u32) << 24 | (bytes[5] as u32) << 16 | (bytes[6] as u32) << 8 | (bytes[7] as u32);
        let ack_num = (bytes[8] as u32) << 24 | (bytes[9] as u32) << 16 | (bytes[10] as u32) << 8 | (bytes[11] as u32);

        let nonce_sum = (bytes[12] & 0b_00000001) != 0;
        let flags = bytes[13];
        let window = (bytes[14] as u16) << 8 | (bytes[15] as u16);
        let checksum = (bytes[16] as u16) << 8 | (bytes[17] as u16);
        let urgent_ptr = {
            const URG_FLAG: u8 = 0b_00100000;
            if flags & URG_FLAG != 0 {
                Some((bytes[18] as u16) << 8 | (bytes[19] as u16))
            }
            else { None }
        };

        let options: Vec<TcpOption> = 
            if options_len == 0 { Vec::new() }
            else {
                let mut opts = Vec::with_capacity(options_len);
                let mut idx: usize = MINIMAL_TCP_HEADER_LEN;
                while idx < header_with_options_len {
                    let kind = bytes[idx];
                    if kind == 0 { break; }
                    idx += 1;
                    if kind == 1 { continue; }
                    let len = (bytes[idx] - 2) as usize;
                    idx += 1;
                    let opt_upper_bound = idx + len;
                    if opt_upper_bound > header_with_options_len  { return Err(PacketError::TcpParseOptionsTooShort(opt_upper_bound, header_with_options_len)); }
                    opts.push(TcpOption { kind: kind, value: bytes[idx..(opt_upper_bound)].to_vec()});
                    idx += len;
                }

                opts
            };

        Ok( TcpHeader {
            src_port: src_port,
            dst_port: dst_port,
            seq_num: seq_num,
            ack_num: ack_num,
            data_offset: data_offset,
            nonce_sum: nonce_sum,
            flags: flags,
            window: window,
            checksum: checksum,
            urgent_ptr: urgent_ptr,
            options: options
        })
    }

    pub fn get_length(&self) -> usize {
        let options_len = ((self.data_offset - 5) * 4) as usize;
        MINIMAL_TCP_HEADER_LEN + options_len
    }

    pub fn get_flags(&self) -> Vec<Flag> {
        const ALL_POSSIBLE_FLAGS_LEN: usize = 8;
        const MASKS_FLAGS: [(u8, Flag); ALL_POSSIBLE_FLAGS_LEN] = [
            (0b10000000, Flag::Cwr),
            (0b01000000, Flag::Ece),
            (0b00100000, Flag::Urg),
            (0b00010000, Flag::Ack),
            (0b00001000, Flag::Psh),
            (0b00000100, Flag::Rst),
            (0b00000010, Flag::Syn),
            (0b00000001, Flag::Fin)
        ];

        let mut flags = Vec::with_capacity(ALL_POSSIBLE_FLAGS_LEN);
        for (mask, flag) in MASKS_FLAGS {
            if self.flags & mask != 0 {
                flags.push(flag);
            }
        }

        flags.shrink_to_fit();
        flags
    }

    pub fn get_source_port(&self) -> u16 {
        self.src_port
    }

    pub fn get_destination_port(&self) -> u16 {
        self.dst_port
    }

    pub fn get_sequence_number(&self) -> u32 {
        self.seq_num
    }

    pub fn get_acknowledgment_number(&self) -> u32 {
        self.ack_num
    }

    pub fn get_nonce_sum(&self) -> bool {
        self.nonce_sum
    }

    pub fn get_window(&self) -> u16 {
        self.window
    }

    pub fn get_checksum(&self) -> u16 {
        self.checksum
    }

    pub fn get_urgent_ptr(&self) -> Option<u16> {
        self.urgent_ptr.clone()
    }

    pub fn get_options(&self) -> &Vec<TcpOption> {
        &self.options
    }
}

#[test]
fn test_tcp_1() {
    let tcp_creation_result = TcpHeader::create(&[0x00, 0x50, 0x01, 0xBB, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x50, 0x02, 0x71, 0x10, 0x12, 0x34, 0x00, 0x00]);
    assert!(tcp_creation_result.is_ok());
    let tcp = tcp_creation_result.unwrap();
    assert_eq!(tcp.src_port, 80);
    assert_eq!(tcp.dst_port, 443);
    assert_eq!(tcp.ack_num, 0);
    assert_eq!(tcp.seq_num, 1);
    assert_eq!(tcp.data_offset, 5);
    assert_eq!(tcp.window, 0x7110);
    assert_eq!(tcp.checksum, 0x1234);
    assert!(!tcp.nonce_sum);
    assert!(tcp.urgent_ptr.is_none());
    assert_eq!(tcp.get_flags(), vec![Flag::Syn]);
    assert!(tcp.options.is_empty());
}

#[test]
fn test_tcp_2() {
    let tcp_creation_result = TcpHeader::create(&[0x00, 0x50, 0x01, 0xBB, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x60, 0x02, 0x71, 0x10, 0x12, 0x34, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01]);
    assert!(tcp_creation_result.is_ok());
    let tcp = tcp_creation_result.unwrap();
    assert_eq!(tcp.src_port, 80);
    assert_eq!(tcp.dst_port, 443);
    assert_eq!(tcp.ack_num, 0);
    assert_eq!(tcp.seq_num, 1);
    assert_eq!(tcp.data_offset, 6);
    assert_eq!(tcp.window, 0x7110);
    assert_eq!(tcp.checksum, 0x1234);
    assert!(!tcp.nonce_sum);
    assert!(tcp.urgent_ptr.is_none());
    assert_eq!(tcp.get_flags(), vec![Flag::Syn]);
    assert!(tcp.options.is_empty());
}

#[test]
fn test_tcp_3() {
    let tcp_creation_result = TcpHeader::create(&[0x00, 0x50, 0x01, 0xBB, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x60, 0x02, 0x71, 0x10, 0x12, 0x34, 0x00, 0x00, 0x02, 0x04, 0x05, 0xB4]);
    assert!(tcp_creation_result.is_ok());
    let tcp = tcp_creation_result.unwrap();
    assert_eq!(tcp.src_port, 80);
    assert_eq!(tcp.dst_port, 443);
    assert_eq!(tcp.ack_num, 0);
    assert_eq!(tcp.seq_num, 1);
    assert_eq!(tcp.data_offset, 6);
    assert_eq!(tcp.window, 0x7110);
    assert_eq!(tcp.checksum, 0x1234);
    assert!(!tcp.nonce_sum);
    assert!(tcp.urgent_ptr.is_none());
    assert_eq!(tcp.get_flags(), vec![Flag::Syn]);
    assert_eq!(tcp.options, vec![TcpOption{ kind: 2, value: vec![0x05, 0xB4]}]);
}

#[test]
fn test_tcp_4() {
    let tcp_creation_result = TcpHeader::create(&[0x00, 0x50, 0x01, 0xBB, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x7B, 0b01010001, 0b00111010, 0x40, 0x00, 0xAB, 0xCD, 0x13, 0x37]);
    assert!(tcp_creation_result.is_ok());
    let tcp = tcp_creation_result.unwrap();
    assert_eq!(tcp.src_port, 80);
    assert_eq!(tcp.dst_port, 443);
    assert_eq!(tcp.ack_num, 123);
    assert_eq!(tcp.seq_num, 1);
    assert_eq!(tcp.data_offset, 5);
    assert_eq!(tcp.window, 0x4000);
    assert_eq!(tcp.checksum, 0xABCD);
    assert!(tcp.nonce_sum);
    assert_eq!(tcp.urgent_ptr, Some(0x1337));
    assert_eq!(tcp.get_flags(), vec![Flag::Urg, Flag::Ack, Flag::Psh, Flag::Syn]);
    assert!(tcp.options.is_empty());
}

#[test]
fn cannot_create_tcp() {
    assert_eq!(TcpHeader::create(&[0x00, 0x50, 0x01]), Err(PacketError::TcpTooShort(3)));
    assert_eq!(TcpHeader::create(&[0x00, 0x50, 0x01, 0xBB, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0b01010011, 0b00000010, 0x71, 0x10, 0x12, 0x34, 0x00, 0x00,]), Err(PacketError::TcpReservedIsNonZero(1)));
    assert_eq!(TcpHeader::create(&[0x00, 0x50, 0x01, 0xBB, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x40, 0x02, 0x71, 0x10, 0x12, 0x34, 0x00, 0x00]), Err(PacketError::TcpDataOffsetWrong(4)));
    assert_eq!(TcpHeader::create(&[0x00, 0x50, 0x01, 0xBB, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x60, 0x02, 0x71, 0x10, 0x12, 0x34, 0x00, 0x00, 0x02, 0x10]), Err(PacketError::TcpTooShort(22)));
}