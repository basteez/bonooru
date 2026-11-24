//! CRC-16/CCITT-FALSE (RevEng CRC-16/IBM-3740): poly 0x1021, init 0xFFFF, no
//! reflection, no final XOR. Sent little-endian on the wire (AD-3). Check
//! value 0x29B1 over ASCII "123456789".

use super::consts::{CRC16_INIT, CRC16_POLY};

/// CRC of `data`.
#[must_use]
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc = CRC16_INIT;
    for &b in data {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ CRC16_POLY
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::consts::CRC16_CHECK;

    #[test]
    fn check_value() {
        assert_eq!(crc16(b"123456789"), CRC16_CHECK);
        assert_eq!(crc16(&[]), 0xFFFF);
    }
}
