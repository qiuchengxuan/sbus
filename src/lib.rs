#![no_std]
#![feature(test)]

#[derive(Default, PartialEq, Debug)]
pub struct SbusData {
    pub channels: [u16; 16],
    pub channel17: bool,
    pub channel18: bool,
    pub frame_lost: bool,
    pub failsafe: bool,
}

#[repr(C)]
pub struct SbusPacket {
    _padding: u8,
    header: u8,
    channel_octets: [u16; 11],
    digital_and_flags: u8,
    footer: u8,
}

pub const SBUS_PACKET_BEGIN: u8 = 0xF;
pub const SBUS_PACKET_SIZE: usize = core::mem::size_of::<SbusPacket>() - 1;

pub fn is_sbus_packet_end(byte: u8) -> bool {
    match byte {
        0x0 => true,
        0x4 => true,
        0x14 => true,
        0x24 => true,
        0x34 => true,
        _ => false,
    }
}

impl SbusPacket {
    pub fn from_bytes<'a>(bytes: &'a [u8]) -> Option<&Self> {
        if bytes.len() < SBUS_PACKET_SIZE {
            return None;
        }
        Some(unsafe { &*(&bytes[0] as *const u8 as *const SbusPacket) })
    }

    pub fn try_parse(&self) -> Option<SbusData> {
        if self.header != SBUS_PACKET_BEGIN || !is_sbus_packet_end(self.footer) {
            return None;
        }
        Some(self.parse())
    }

    pub fn parse(&self) -> SbusData {
        const SHIFT: [u8; 16] = [0, 5, 10, 15, 4, 9, 14, 3, 8, 13, 2, 7, 12, 1, 6, 11];
        const INDEX: [u8; 16] = [0, 1, 2, 3, 3, 4, 5, 5, 6, 7, 7, 8, 9, 9, 10, 10];

        let mut data = SbusData::default();
        let mut bits: u32 = 0;
        for i in 0..16 {
            let octet = u16::from_le(self.channel_octets[INDEX[i] as usize]) as u32;
            bits |= octet << (SHIFT[i] as usize);
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

#[cfg(test)]
mod tests {

    extern crate test;
    use test::Bencher;

    #[test]
    fn test_sbus() {
        use super::{SbusData, SbusPacket, SBUS_PACKET_SIZE};

        assert_eq!(SBUS_PACKET_SIZE, 25);
        let bytes: [u8; SBUS_PACKET_SIZE + 1] = [
            0x00, 0x0F, 0xE0, 0x03, 0x1F, 0x58, 0xC0, 0x07, 0x16, 0xB0, 0x80, 0x05, 0x2C, 0x60,
            0x01, 0x0B, 0xF8, 0xC0, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00,
        ];
        let sbus_packet: &SbusPacket = SbusPacket::from_bytes(&bytes[..]).unwrap();
        let result = sbus_packet.try_parse();
        assert_eq!(
            result,
            Some(SbusData {
                channels: [992, 992, 352, 992, 352, 352, 352, 352, 352, 352, 992, 992, 0, 0, 0, 0],
                channel17: false,
                channel18: false,
                frame_lost: false,
                failsafe: false,
            })
        )
    }

    #[bench]
    fn bench_sbus(b: &mut Bencher) {
        use super::{SbusPacket, SBUS_PACKET_SIZE};

        let bytes: [u8; SBUS_PACKET_SIZE + 1] = [
            0x00, 0x0F, 0xE0, 0x03, 0x1F, 0x58, 0xC0, 0x07, 0x16, 0xB0, 0x80, 0x05, 0x2C, 0x60,
            0x01, 0x0B, 0xF8, 0xC0, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00,
        ];
        let sbus_packet: &SbusPacket = SbusPacket::from_bytes(&bytes[..]).unwrap();
        b.iter(|| sbus_packet.parse());
    }
}
