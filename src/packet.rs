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

#[cfg(test)]
mod tests {
    #[test]
    fn test_sbus_packet() {
        use super::{Data, Packet, SBUS_PACKET_SIZE};

        assert_eq!(SBUS_PACKET_SIZE, 25);
        let bytes =
            hex!("00 0F E0 03 1F 58 C0 07 16 B0 80 05 2C 60 01 0B F8 C0 07 00 00 00 00 00 03 00");

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
}
