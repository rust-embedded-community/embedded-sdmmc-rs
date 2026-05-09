//! # CID support module

use arbitrary_int::{u7, u12, u40};

use crate::sdcard::crc7;

/// Checksum is invalid.
#[derive(Debug, PartialEq, Eq, Copy, Clone, thiserror::Error)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[error("checksum is invalid")]
pub struct ChecksumInvalidError;

/// Card IDentification (CID) register structure.
#[bitbybit::bitfield(u128, debug, defmt_fields(feature = "defmt"))]
pub struct Cid {
    /// MID.
    #[bits(120..=127, rw)]
    manufacturer_id: u8,
    /// OID.
    #[bits(104..=119, rw)]
    oem_id: u16,
    /// PVM.
    #[bits(64..=103, rw)]
    product_name_raw: u40,
    /// PSN.
    #[bits(24..=55, rw)]
    product_serial_number: u32,
    /// MDT.
    #[bits(8..=19, rw)]
    manufacturing_date: u12,
    /// CRC.
    #[bits(1..=7, rw)]
    crc7: u7,
}

impl Cid {
    /// Construct a [Cid] from a raw byte slice with 16 bytes.
    ///
    /// Also verifies the CRC7 checksum.
    pub fn new(raw: &[u8; 16]) -> Result<Cid, ChecksumInvalidError> {
        let cid = Cid::new_with_raw_value(u128::from_be_bytes(*raw));
        if !cid.verify_crc7() {
            return Err(ChecksumInvalidError);
        }
        Ok(cid)
    }

    /// Construct a [Cid] without verifying the checksum.
    pub fn new_unchecked(raw: &[u8; 16]) -> Cid {
        Cid::new_with_raw_value(u128::from_be_bytes(*raw))
    }

    /// Verify CRC7 checksum.
    pub fn verify_crc7(&self) -> bool {
        let raw_bytes = self.raw_value().to_be_bytes();
        let calculated = crc7(&raw_bytes[0..15]);
        self.crc7().value() == calculated
    }

    /// Product name as a byte array.
    #[inline]
    pub fn product_name_bytes(&self) -> [u8; 5] {
        self.product_name_raw().to_be_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_test() {
        // CID retrieved from a real SD card.
        let raw_cid = [
            0x12, 0x34, 0x56, 0x41, 0x53, 0x54, 0x43, 0x0, 0x20, 0x0, 0x0, 0xc, 0xef, 0x1, 0x65,
            0xef,
        ];
        // This also verifies the checksum.
        let cid = Cid::new(&raw_cid);
        assert!(cid.is_ok());

        let cid = cid.unwrap();
        // https://www.bahjeez.com/sd-card-manufacturer-ids/: Patriot.
        assert_eq!(cid.manufacturer_id(), 0x12);
        assert_eq!(cid.oem_id(), 0x3456);

        let product_name_raw = cid.product_name_bytes();
        let product_name = core::str::from_utf8(&product_name_raw).unwrap();
        assert_eq!(product_name, "ASTC\0");
    }

    #[test]
    fn basic_test_unchecked() {
        // CID retrieved from a real SD card.
        let raw_cid = [
            0x12, 0x34, 0x56, 0x41, 0x53, 0x54, 0x43, 0x0, 0x20, 0x0, 0x0, 0xc, 0xef, 0x1, 0x65,
            0x00,
        ];
        // Invalid checksum ignored.
        let cid = Cid::new_unchecked(&raw_cid);
        assert!(!cid.verify_crc7());

        // https://www.bahjeez.com/sd-card-manufacturer-ids/: Patriot.
        assert_eq!(cid.manufacturer_id(), 0x12);
        assert_eq!(cid.oem_id(), 0x3456);

        let product_name_raw = cid.product_name_bytes();
        let product_name = core::str::from_utf8(&product_name_raw).unwrap();
        assert_eq!(product_name, "ASTC\0");
    }
}
