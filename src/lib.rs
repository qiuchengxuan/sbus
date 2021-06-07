#![cfg_attr(not(test), no_std)]

#[cfg(test)]
#[macro_use]
extern crate hex_literal;

#[derive(Default, PartialEq, Debug)]
pub struct Data {
    pub channels: [u16; 16],
    pub channel17: bool,
    pub channel18: bool,
    pub frame_lost: bool,
    pub failsafe: bool,
}

#[repr(C)]
pub struct Packet {
    _padding: u8,
    header: u8,
    channel_words: [u16; 11],
    digital_and_flags: u8,
    footer: u8,
}

pub const SBUS_PACKET_BEGIN: u8 = 0xF;
pub const SBUS_PACKET_SIZE: usize = core::mem::size_of::<Packet>() - 1;

pub fn is_sbus_packet_end(byte: u8) -> bool {
    match byte {
        0x0 => true,  // S.BUS 1
        0x4 => true,  // S.BUS 2 receiver voltage
        0x14 => true, // S.BUS 2 GPS/baro
        0x24 => true, // Unknown SBUS2 data
        0x34 => true, // Unknown SBUS2 data
        _ => false,
    }
}

impl Packet {
    pub fn parse(&self) -> Data {
        const SHIFT: [u8; 16] = [0, 5, 10, 15, 4, 9, 14, 3, 8, 13, 2, 7, 12, 1, 6, 11];
        const INDEX: [u8; 16] = [0, 1, 2, 3, 3, 4, 5, 5, 6, 7, 7, 8, 9, 9, 10, 10];

        let mut data = Data::default();
        let mut bits: u32 = 0;
        for i in 0..16 {
            let word = u16::from_le(self.channel_words[INDEX[i] as usize]) as u32;
            bits |= word << (SHIFT[i] as usize);
            data.channels[i] = bits as u16 & ((1 << 11) - 1);
            bits >>= 11;
        }

        data.channel17 = (self.digital_and_flags & (1 << 7)) > 0;
        data.channel18 = (self.digital_and_flags & (1 << 6)) > 0;
        data.frame_lost = (self.digital_and_flags & (1 << 5)) > 0;
        data.failsafe = (self.digital_and_flags & (1 << 4)) > 0;
        data
    }
}

pub struct Receiver {
    packet: [u8; 1 + SBUS_PACKET_SIZE],
    size: usize,
}

impl Receiver {
    pub fn new() -> Self {
        Self { packet: [0u8; 1 + SBUS_PACKET_SIZE], size: 0 }
    }

    fn continue_receive(&mut self, bytes: &[u8]) -> Option<Data> {
        let offset = SBUS_PACKET_SIZE - self.size;
        if is_sbus_packet_end(bytes[offset - 1]) {
            self.packet[1 + self.size..].copy_from_slice(&bytes[..offset]);
            self.size = 0;
            let packet: &Packet = unsafe { core::mem::transmute(&self.packet) };
            return Some(packet.parse());
        }
        for i in 1..self.size {
            let size = self.size - i;
            let remain_size = SBUS_PACKET_SIZE - size;
            let last = bytes[remain_size - 1];
            if self.packet[1 + i] == SBUS_PACKET_BEGIN && is_sbus_packet_end(last) {
                self.packet.copy_within(1 + i..1 + self.size, 1);
                self.packet[1 + size..].copy_from_slice(&bytes[..remain_size]);
                self.size = 0;
                let packet: &Packet = unsafe { core::mem::transmute(&self.packet) };
                return Some(packet.parse());
            }
        }
        self.size = 0;
        None
    }

    // Assuming only one or none SBUS packet exists
    pub fn receive(&mut self, bytes: &[u8]) -> Option<Data> {
        assert!(bytes.len() >= SBUS_PACKET_SIZE);
        if self.size > 0 {
            if let Some(data) = self.continue_receive(bytes) {
                return Some(data);
            }
        }
        for i in 0..bytes.len() {
            if bytes[i] == SBUS_PACKET_BEGIN {
                if i + SBUS_PACKET_SIZE <= bytes.len() {
                    self.packet[1..].copy_from_slice(&bytes[i..i + SBUS_PACKET_SIZE]);
                    let packet: &Packet = unsafe { core::mem::transmute(&self.packet) };
                    return Some(packet.parse());
                } else {
                    self.size = bytes.len() - i;
                    self.packet[1..1 + self.size].copy_from_slice(&bytes[i..]);
                    break;
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_sbus() {
        use super::{Data, Packet, SBUS_PACKET_SIZE};

        assert_eq!(SBUS_PACKET_SIZE, 25);
        let bytes: [u8; SBUS_PACKET_SIZE + 1] = hex!(
            "00 0F E0 03 1F 58 C0 07 16 B0 80 05 2C 60 01 0B
             F8 C0 07 00 00 00 00 00 03 00"
        );

        let sbus_packet: &Packet = unsafe { core::mem::transmute(&bytes) };
        assert_eq!(
            sbus_packet.parse(),
            Data {
                channels: [992, 992, 352, 992, 352, 352, 352, 352, 352, 352, 992, 992, 0, 0, 0, 0],
                channel17: false,
                channel18: false,
                frame_lost: false,
                failsafe: false,
            }
        )
    }

    #[test]
    fn test_received_some() {
        use super::{Receiver, SBUS_PACKET_SIZE};

        let mut receiver = Receiver::new();
        let bytes: [u8; SBUS_PACKET_SIZE] =
            hex!("0F E0 03 1F 58 C0 07 16 B0 80 05 2C 60 01 0B F8 C0 07 00 00 00 00 00 03 00");
        assert!(receiver.receive(&bytes).is_some());
    }

    #[test]
    fn test_received_none() {
        use super::{Receiver, SBUS_PACKET_SIZE};

        let mut receiver = Receiver::new();
        let bytes: [u8; SBUS_PACKET_SIZE] =
            hex!("FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF");
        assert!(receiver.receive(&bytes).is_none());
        assert_eq!(receiver.size, 0);
    }

    #[test]
    fn test_partially_receive() {
        use super::{Receiver, SBUS_PACKET_SIZE};

        let mut receiver = Receiver::new();

        let bytes: [u8; SBUS_PACKET_SIZE] =
            hex!("00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 0F 00");
        assert!(receiver.receive(&bytes).is_none());
        assert_eq!(receiver.size, 2);

        let bytes: [u8; SBUS_PACKET_SIZE] =
            hex!("FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF 00 FF FF");
        assert!(receiver.receive(&bytes).is_some());
        assert_eq!(receiver.size, 0);
    }

    #[test]
    fn test_header_not_sbus() {
        use super::{Receiver, SBUS_PACKET_SIZE};

        let mut receiver = Receiver::new();

        let bytes: [u8; SBUS_PACKET_SIZE] =
            hex!("00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 0F 0F 01");
        assert!(receiver.receive(&bytes).is_none());
        assert_eq!(receiver.size, 3);

        let bytes: [u8; SBUS_PACKET_SIZE] =
            hex!("FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF 02 00 FF FF");
        assert!(receiver.receive(&bytes).is_some());
        assert_eq!(receiver.size, 0);
        assert_eq!(receiver.packet[1..3], [0xF, 0x1]);
        assert_eq!(receiver.packet[1 + SBUS_PACKET_SIZE - 3..], [0xFF, 0x2, 0x0]);
    }
}
