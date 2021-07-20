use crate::packet::{Data, Packet, SBUS_PACKET_BEGIN, SBUS_PACKET_SIZE};

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

#[derive(Debug)]
pub struct Receiver {
    packet: [u8; 1 + SBUS_PACKET_SIZE],
    size: usize,
}

impl Receiver {
    pub fn new() -> Self {
        Self { packet: [0u8; 1 + SBUS_PACKET_SIZE], size: 0 }
    }

    fn find_partial_packet(&mut self, bytes: &[u8]) {
        for i in 0..bytes.len() {
            if bytes[i] == SBUS_PACKET_BEGIN {
                self.size = bytes.len() - i;
                self.packet[1..1 + self.size].copy_from_slice(&bytes[i..]);
                break;
            }
        }
    }

    fn continue_receive(&mut self, bytes: &[u8]) -> Option<Data> {
        let packet = &mut self.packet[1..];
        for offset in 0..self.size {
            if packet[offset] != SBUS_PACKET_BEGIN {
                continue;
            }
            let size = self.size - offset;
            let remain_size = SBUS_PACKET_SIZE - size;
            if bytes.len() < remain_size {
                packet[size..size + bytes.len()].copy_from_slice(bytes);
                self.size += bytes.len();
                return None;
            }
            if is_sbus_packet_end(bytes[remain_size - 1]) {
                packet.copy_within(offset..self.size, 0);
                packet[size..].copy_from_slice(&bytes[..remain_size]);
                let packet: &Packet = unsafe { core::mem::transmute(&self.packet) };
                let data = Some(packet.parse());
                self.size = 0;
                self.find_partial_packet(&bytes[remain_size..]);
                return data;
            }
        }
        self.size = 0;
        None
    }

    /// Must be chunk of SBUS PACKET SIZE or less
    pub fn receive(&mut self, bytes: &[u8]) -> Option<Data> {
        assert!(bytes.len() <= SBUS_PACKET_SIZE);
        if self.size > 0 {
            if let Some(data) = self.continue_receive(bytes) {
                return Some(data);
            }
        }
        let mut index = 0;
        if bytes.len() == SBUS_PACKET_SIZE {
            if bytes[0] == SBUS_PACKET_BEGIN && is_sbus_packet_end(bytes[SBUS_PACKET_SIZE - 1]) {
                self.packet[1..].copy_from_slice(bytes);
                let packet: &Packet = unsafe { core::mem::transmute(&self.packet) };
                return Some(packet.parse());
            }
            index = 1;
        }
        self.find_partial_packet(&bytes[index..]);
        None
    }

    pub fn reset(&mut self) {
        self.size = 0;
    }
}

#[cfg(test)]
mod tests {
    use crate::packet::SBUS_PACKET_SIZE;

    const SBUS_SAMPLE_PACKET: [u8; SBUS_PACKET_SIZE] =
        hex!("0F E0 03 1F 58 C0 07 16 B0 80 05 2C 60 01 0B F8 C0 07 00 00 00 00 00 03 00");

    #[test]
    fn test_received_some() {
        let mut receiver = super::Receiver::new();
        assert_eq!(receiver.receive(&SBUS_SAMPLE_PACKET).unwrap().channels[0], 992);
    }

    #[test]
    fn test_received_none() {
        let mut receiver = super::Receiver::new();
        assert!(receiver.receive(&[0xFFu8; SBUS_PACKET_SIZE]).is_none());
    }

    #[test]
    fn test_partially_receive() {
        for i in 1..SBUS_PACKET_SIZE {
            let mut receiver = super::Receiver::new();
            let mut bytes = [0u8; SBUS_PACKET_SIZE];
            bytes[..i].copy_from_slice(&SBUS_SAMPLE_PACKET[SBUS_PACKET_SIZE - i..]);
            bytes[i..].copy_from_slice(&SBUS_SAMPLE_PACKET[..SBUS_PACKET_SIZE - i]);
            assert!(receiver.receive(&bytes).is_none());
            assert_eq!(receiver.receive(&bytes).unwrap().channels[0], 992);
        }
    }

    #[test]
    fn test_footer_not_sbus() {
        let mut receiver = super::Receiver::new();
        let b = hex!("0F 00 FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF 01");
        assert!(receiver.receive(&b).is_none());
    }

    #[test]
    fn test_header_not_sbus() {
        let mut receiver = super::Receiver::new();

        let b = hex!("00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 0F 0F E0");
        assert!(receiver.receive(&b).is_none());

        let b = hex!("03 1F 58 C0 07 16 B0 80 05 2C 60 01 0B F8 C0 07 00 00 00 00 00 03 00 FF FF");
        assert_eq!(receiver.receive(&b).unwrap().channels[0], 992);
    }

    #[test]
    fn test_fragment() {
        let mut receiver = super::Receiver::new();

        assert!(receiver.receive(&[0xF]).is_none());
        let b = hex!("00 FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF");
        assert!(receiver.receive(&b).is_none());
        assert!(receiver.receive(&[0]).is_some());
    }
}
