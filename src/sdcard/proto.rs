//! Constants from the SD Specifications
//!
//! Based on SdFat, under the following terms:
//!
//! > Copyright (c) 2011-2018 Bill Greiman
//! > This file is part of the SdFat library for SD memory cards.
//! >
//! > MIT License
//! >
//! > Permission is hereby granted, free of charge, to any person obtaining a
//! > copy of this software and associated documentation files (the "Software"),
//! > to deal in the Software without restriction, including without limitation
//! > the rights to use, copy, modify, merge, publish, distribute, sublicense,
//! > and/or sell copies of the Software, and to permit persons to whom the
//! > Software is furnished to do so, subject to the following conditions:
//! >
//! > The above copyright notice and this permission notice shall be included
//! > in all copies or substantial portions of the Software.
//! >
//! > THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
//! > OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! > FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! > AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! > LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//! > FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//! > DEALINGS IN THE SOFTWARE.

//==============================================================================

// Possible errors the SD card can return

use arbitrary_int::{traits::Integer, u2, u3, u7, u12, u22, u28};

/// Card indicates last operation was a success
pub const ERROR_OK: u8 = 0x00;

//==============================================================================

// SD Card Commands

/// GO_IDLE_STATE - init card in spi mode if CS low
pub const CMD0: u8 = 0x00;
/// SEND_IF_COND - verify SD Memory Card interface operating condition.*/
pub const CMD8: u8 = 0x08;
/// SEND_CSD - read the Card Specific Data (CSD register)
pub const CMD9: u8 = 0x09;
/// STOP_TRANSMISSION - end multiple block read sequence
pub const CMD12: u8 = 0x0C;
/// SEND_STATUS - read the card status register
pub const CMD13: u8 = 0x0D;
/// READ_SINGLE_BLOCK - read a single data block from the card
pub const CMD17: u8 = 0x11;
/// READ_MULTIPLE_BLOCK - read a multiple data blocks from the card
pub const CMD18: u8 = 0x12;
/// WRITE_BLOCK - write a single data block to the card
pub const CMD24: u8 = 0x18;
/// WRITE_MULTIPLE_BLOCK - write blocks of data until a STOP_TRANSMISSION
pub const CMD25: u8 = 0x19;
/// APP_CMD - escape for application specific command
pub const CMD55: u8 = 0x37;
/// READ_OCR - read the OCR register of a card
pub const CMD58: u8 = 0x3A;
/// CRC_ON_OFF - enable or disable CRC checking
pub const CMD59: u8 = 0x3B;
/// Pre-erased before writing
///
/// > It is recommended using this command preceding CMD25, some of the cards will be faster for Multiple
/// > Write Blocks operation. Note that the host should send ACMD23 just before WRITE command if the host
/// > wants to use the pre-erased feature
pub const ACMD23: u8 = 0x17;
/// SD_SEND_OP_COMD - Sends host capacity support information and activates
/// the card's initialization process
pub const ACMD41: u8 = 0x29;

//==============================================================================

/// status for card in the ready state
pub const R1_READY_STATE: u8 = 0x00;

/// status for card in the idle state
pub const R1_IDLE_STATE: u8 = 0x01;

/// status bit for illegal command
pub const R1_ILLEGAL_COMMAND: u8 = 0x04;

/// start data token for read or write single block*/
pub const DATA_START_BLOCK: u8 = 0xFE;

/// stop token for write multiple blocks*/
pub const STOP_TRAN_TOKEN: u8 = 0xFD;

/// start data token for write multiple blocks*/
pub const WRITE_MULTIPLE_TOKEN: u8 = 0xFC;

/// mask for data response tokens after a write block operation
pub const DATA_RES_MASK: u8 = 0x1F;

/// write data accepted token
pub const DATA_RES_ACCEPTED: u8 = 0x05;

/// Card Specific Data
#[derive(Debug)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
pub enum Csd {
    /// A version 1 CSD
    V1(CsdV1),
    /// A version 2 CSD
    V2(CsdV2),
    /// A version 3 CSD
    V3(CsdV3),
}

/// The CSD structure field is invalid.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum CsdCreationError {
    /// Invalid CSD structure field.
    #[error("invalid CSD structure field")]
    InvalidCsdStructureField,
    /// Invalid CRC7 checksum.
    #[error("invalid CRC7 checksum")]
    Checksum,
}

impl Csd {
    /// Construct a [Csd] from a raw byte slice with 16 bytes.
    pub fn new(raw: &[u8; 16]) -> Result<Csd, CsdCreationError> {
        let csd_structure_raw = (raw[0] >> 6) & 0b11;

        let csd = if csd_structure_raw == CsdStructure::CsdV1 as u8 {
            Csd::V1(CsdV1::from_be_bytes(raw))
        } else if csd_structure_raw == CsdStructure::CsdV2 as u8 {
            Csd::V2(CsdV2::from_be_bytes(raw))
        } else if csd_structure_raw == CsdStructure::CsdV3 as u8 {
            Csd::V3(CsdV3::from_be_bytes(raw))
        } else {
            return Err(CsdCreationError::InvalidCsdStructureField);
        };
        if !csd.verify_crc7() {
            return Err(CsdCreationError::Checksum);
        }
        Ok(csd)
    }

    /// Verify CRC7 checksum.
    pub fn verify_crc7(&self) -> bool {
        match self {
            Csd::V1(csd_v1) => csd_v1.verify_crc7(),
            Csd::V2(csd_v2) => csd_v2.verify_crc7(),
            Csd::V3(csd_v3) => csd_v3.verify_crc7(),
        }
    }
}

/// CSD_STRUCTURE field according to SD spec 5.3.1.
#[bitbybit::bitenum(u2, exhaustive = false)]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
pub enum CsdStructure {
    /// CSD Version 1.0, Standard Capacity Card.
    CsdV1 = 0,
    /// CSD Version 2.0, High and Extended Capacity Card.
    CsdV2 = 1,
    /// CSD Version 3.0, Ultra Capacity (SDUC)
    CsdV3 = 2,
}

/// READ_BL_LEN field for CSD version 1 according to SD spec 5.3.3.
#[bitbybit::bitenum(u4, exhaustive = false)]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
pub enum BlockLengthSelectV1 {
    /// 512 bytes.
    _512 = 9,
    /// 1024 bytes.
    _1024 = 10,
    /// 2048 bytes.
    _2048 = 11,
}

/// READ_BL_LEN field for CSD version 2 and 3 according to SD spec.
#[bitbybit::bitenum(u4, exhaustive = false)]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
pub enum BlockLengthSelectV2AndV3 {
    /// 512 bytes.
    _512 = 9,
}

/// C_SIZE_MULT field according to SD spec 5.3.5.
#[bitbybit::bitenum(u3, exhaustive = true)]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
pub enum SizeMultiplierSelect {
    /// Multiplier of 4.
    _4 = 0,
    /// Multiplier of 8.
    _8 = 1,
    /// Multiplier of 16.
    _16 = 2,
    /// Multiplier of 32.
    _32 = 3,
    /// Multiplier of 64.
    _64 = 4,
    /// Multiplier of 128.
    _128 = 5,
    /// Multiplier of 256.
    _256 = 6,
    /// Multiplier of 512.
    _512 = 7,
}

impl SizeMultiplierSelect {
    /// Multiplier as actual multiplication value.
    #[inline]
    pub fn multiplier(&self) -> usize {
        match self {
            SizeMultiplierSelect::_4 => 4,
            SizeMultiplierSelect::_8 => 8,
            SizeMultiplierSelect::_16 => 16,
            SizeMultiplierSelect::_32 => 32,
            SizeMultiplierSelect::_64 => 64,
            SizeMultiplierSelect::_128 => 128,
            SizeMultiplierSelect::_256 => 256,
            SizeMultiplierSelect::_512 => 512,
        }
    }
}

bitflags::bitflags! {
    /// CCC field according to SD card spec 5.3.2.
    ///
    /// The table of command classes is specified in Table 4-21.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CardCommandClasses: u16 {
        /// Basic command class.
        const BASIC = 0b000000000001;
        /// COMM and Queue.
        const COMM_AND_QUEUE = 0b000000000010;
        /// Block Read.
        const BLOCK_READ = 0b000000000100;
        /// Reserved block.
        const RESERVED_3 = 0b000000001000;
        /// Block Write.
        const BLOCK_WRITE = 0b000000010000;
        /// Erase.
        const ERASE = 0b000000100000;
        /// Write protection.
        const WRITE_PROTECTION = 0b000001000000;
        /// Lock card.
        const LOCK_CARD = 0b000010000000;
        /// Application specific.
        const APP_SPECIFIC = 0b000100000000;
        /// I/O mode.
        const IO_MODE = 0b001000000000;
        /// Switch
        const SWITCH = 0b010000000000;
        /// Extension.
        const EXTENSION = 0b100000000000;
    }
}

/// CSD V1 register structure.
#[bitbybit::bitfield(u128, debug, defmt_fields(feature = "defmt-log"))]
pub struct CsdV1 {
    /// CSD_STRUCTURE field.
    #[bits(126..=127, r)]
    csd_structure: Option<CsdStructure>,
    /// TAAC field.
    #[bits(112..=119, r)]
    data_read_access_time1: u8,
    /// NSAC field.
    #[bits(104..=111, r)]
    data_read_access_time2: u8,
    /// TRAN_SPEED field.
    #[bits(96..=103, r)]
    max_data_transfer_rate: u8,
    /// CCC field.
    #[bits(84..=95, r)]
    card_command_classes_raw: u12,
    /// READ_BL_LEN field.
    #[bits(80..=83, r)]
    read_block_length: Option<BlockLengthSelectV1>,
    /// READ_BL_PARTIAL field.
    #[bit(79, r)]
    partial_blocks_for_read_allowed: bool,
    /// WRITE_BLK_MISALIGN field.
    #[bit(78, r)]
    write_block_misalignment: bool,
    /// READ_BLK_MISALIGN field.
    #[bit(78, r)]
    read_block_misalignment: bool,
    /// DSR_IMP field.
    #[bit(77, r)]
    dsr_implemented: bool,
    /// C_SIZE field.
    #[bits(62..=73, r)]
    device_size: u12,
    /// VDD_R_CURR_MIN field.
    #[bits(59..=61, r)]
    max_read_current_vdd_min: u3,
    /// VDD_R_CURR_MAX field.
    #[bits(56..=58, r)]
    max_read_current_vdd_max: u3,
    /// VDD_W_CURR_MIN field.
    #[bits(53..=55, r)]
    max_write_current_vdd_min: u3,
    /// VDD_W_CURR_MAX field.
    #[bits(50..=52, r)]
    max_write_current_vdd_max: u3,
    /// C_SIZE_MULT field.
    #[bits(47..=49, r)]
    device_size_multiplier: SizeMultiplierSelect,
    /// ERASE_BLK_EN field.
    #[bit(46, r)]
    erase_single_block_enabled: bool,
    /// SECTOR_SIZE field.
    #[bits(39..=45, r)]
    erase_sector_size: u7,
    /// WR_GRP_SIZE field.
    #[bits(32..=38, r)]
    write_protect_group_size: u7,
    /// WR_GRP_ENABLE field.
    #[bit(31, r)]
    write_protect_group_enable: bool,
    /// R2W_FACTOR field.
    #[bits(26..=28, r)]
    write_speed_factor: u3,
    /// WRITE_BL_LEN field.
    #[bits(22..=25, r)]
    write_block_length: Option<BlockLengthSelectV1>,
    /// WRITE_BL_PARTIAL field.
    #[bit(21, r)]
    partial_blocks_for_write_allowed: bool,
    /// FILE_FORMAT_GROUP field.
    #[bit(15, rw)]
    file_format_group_set: bool,
    /// COPY field.
    #[bit(14, rw)]
    copy_flag_set: bool,
    /// PERM_WRITE_PROTECT field.
    #[bit(13, rw)]
    permanent_write_protection: bool,
    /// TEMP_WRITE_PROTECT field.
    #[bit(12, rw)]
    temporary_write_protection: bool,
    /// FILE_FORMAT field.
    #[bits(10..=11, rw)]
    file_format: u2,
    /// WP_UPC field.
    #[bit(9, rw)]
    write_protection_until_power_cycle: bool,
    /// CRC field - CRC7 checksum.
    #[bits(1..=7, rw)]
    crc: u7,
}

impl CsdV1 {
    /// Create [Self] from a raw byte slice.
    pub fn from_be_bytes(bytes: &[u8; 16]) -> Self {
        Self::new_with_raw_value(u128::from_be_bytes(*bytes))
    }

    /// CCC field as type supporting bitflags.
    #[inline]
    pub fn card_command_classes(&self) -> CardCommandClasses {
        // Unwrap okay, we create this from a 12 bit value.
        CardCommandClasses::from_bits(self.card_command_classes_raw().value()).unwrap()
    }

    /// Returns the card capacity in bytes
    pub fn card_capacity_bytes(&self) -> u64 {
        let multiplier = self.device_size_multiplier() as u64
            + self
                .read_block_length()
                .map_or_else(|err| err.as_u64(), |v| v as u64)
            + 2;
        (u64::from(self.device_size()) + 1) << multiplier
    }

    /// Returns the card capacity in 512-byte blocks
    pub fn card_capacity_blocks(&self) -> u32 {
        let multiplier = self.device_size_multiplier() as u32
            + self
                .read_block_length()
                .map_or_else(|err| err.as_u32(), |v| v as u32)
            - 7;
        (self.device_size().as_u32() + 1) << multiplier
    }

    /// Verify CRC7 checksum.
    pub fn verify_crc7(&self) -> bool {
        let raw_bytes = self.raw_value().to_be_bytes();
        let calculated = crc7(&raw_bytes[0..15]);
        self.crc().value() == calculated
    }
}

/// CSD V2 register structure.
#[bitbybit::bitfield(u128, debug, defmt_fields(feature = "defmt-log"))]
pub struct CsdV2 {
    /// CSD_STRUCTURE field.
    #[bits(126..=127, r)]
    csd_structure: Option<CsdStructure>,
    /// TAAC field.
    #[bits(112..=119, r)]
    data_read_access_time1: u8,
    /// NSAC field.
    #[bits(104..=111, r)]
    data_read_access_time2: u8,
    /// TRAN_SPEED field.
    #[bits(96..=103, r)]
    max_data_transfer_rate: u8,
    /// CCC field.
    #[bits(84..=95, r)]
    card_command_classes_raw: u12,
    /// READ_BL_LEN field.
    #[bits(80..=83, r)]
    read_block_length: Option<BlockLengthSelectV2AndV3>,
    /// READ_BL_PARTIAL field.
    #[bit(79, r)]
    partial_blocks_for_read_allowed: bool,
    /// WRITE_BLK_MISALIGN field.
    #[bit(78, r)]
    write_block_misalignment: bool,
    /// READ_BLK_MISALIGN field.
    #[bit(78, r)]
    read_block_misalignment: bool,
    /// DSR_IMP field.
    #[bit(77, r)]
    dsr_implemented: bool,
    /// C_SIZE field.
    #[bits(48..=69, r)]
    device_size: u22,
    /// ERASE_BLK_EN field.
    #[bit(46, r)]
    erase_single_block_enabled: bool,
    /// SECTOR_SIZE field.
    #[bits(39..=45, r)]
    erase_sector_size: u7,
    /// WR_GRP_SIZE field.
    #[bits(32..=38, r)]
    write_protect_group_size: u7,
    /// WR_GRP_ENABLE field.
    #[bit(31, r)]
    write_protect_group_enable: bool,
    /// R2W_FACTOR field.
    #[bits(26..=28, r)]
    write_speed_factor: u3,
    /// WRITE_BL_LEN field.
    #[bits(22..=25, r)]
    write_block_length: Option<BlockLengthSelectV2AndV3>,
    /// WRITE_BL_PARTIAL field.
    #[bit(21, r)]
    partial_blocks_for_write_allowed: bool,
    /// FILE_FORMAT_GROUP field.
    #[bit(15, rw)]
    file_format_group_set: bool,
    /// COPY field.
    #[bit(14, rw)]
    copy_flag_set: bool,
    /// PERM_WRITE_PROTECT field.
    #[bit(13, rw)]
    permanent_write_protection: bool,
    /// TEMP_WRITE_PROTECT field.
    #[bit(12, rw)]
    temporary_write_protection: bool,
    /// FILE_FORMAT field. Should be 0 and host should not use the field.
    #[bits(10..=11, rw)]
    file_format: u2,
    /// WP_UPC field.
    #[bit(9, rw)]
    write_protection_until_power_cycle: bool,
    /// CRC field - CRC7 checksum.
    #[bits(1..=7, rw)]
    crc: u7,
}

impl CsdV2 {
    /// Create [Self] from a raw byte slice.
    pub fn from_be_bytes(bytes: &[u8; 16]) -> Self {
        Self::new_with_raw_value(u128::from_be_bytes(*bytes))
    }

    /// Returns the card capacity in bytes
    pub fn card_capacity_bytes(&self) -> u64 {
        (u64::from(self.device_size()) + 1) * 512 * 1024
    }

    /// Returns the card capacity in 512-byte blocks
    pub fn card_capacity_blocks(&self) -> u32 {
        (self.device_size().as_u32() + 1) * 1024
    }

    /// Verify CRC7 checksum.
    pub fn verify_crc7(&self) -> bool {
        let raw_bytes = self.raw_value().to_be_bytes();
        let calculated = crc7(&raw_bytes[0..15]);
        self.crc().value() == calculated
    }
}

/// CSD V3 register structure.
#[bitbybit::bitfield(u128, debug, defmt_fields(feature = "defmt-log"))]
pub struct CsdV3 {
    /// CSD_STRUCTURE field.
    #[bits(126..=127, r)]
    csd_structure: Option<CsdStructure>,
    /// TAAC field.
    #[bits(112..=119, r)]
    data_read_access_time1: u8,
    /// NSAC field.
    #[bits(104..=111, r)]
    data_read_access_time2: u8,
    /// TRAN_SPEED field.
    #[bits(96..=103, r)]
    max_data_transfer_rate: u8,
    /// CCC field.
    #[bits(84..=95, r)]
    card_command_classes_raw: u12,
    /// READ_BL_LEN field.
    #[bits(80..=83, r)]
    read_block_length: Option<BlockLengthSelectV2AndV3>,
    /// READ_BL_PARTIAL field.
    #[bit(79, r)]
    partial_blocks_for_read_allowed: bool,
    /// WRITE_BLK_MISALIGN field.
    #[bit(78, r)]
    write_block_misalignment: bool,
    /// READ_BLK_MISALIGN field.
    #[bit(78, r)]
    read_block_misalignment: bool,
    /// DSR_IMP field.
    #[bit(77, r)]
    dsr_implemented: bool,
    /// C_SIZE field.
    #[bits(48..=75, r)]
    device_size: u28,
    /// ERASE_BLK_EN field.
    #[bit(46, r)]
    erase_single_block_enabled: bool,
    /// SECTOR_SIZE field.
    #[bits(39..=45, r)]
    erase_sector_size: u7,
    /// WR_GRP_SIZE field.
    #[bits(32..=38, r)]
    write_protect_group_size: u7,
    /// WR_GRP_ENABLE field.
    #[bit(31, r)]
    write_protect_group_enable: bool,
    /// R2W_FACTOR field.
    #[bits(26..=28, r)]
    write_speed_factor: u3,
    /// WRITE_BL_LEN field.
    #[bits(22..=25, r)]
    write_block_length: Option<BlockLengthSelectV2AndV3>,
    /// WRITE_BL_PARTIAL field.
    #[bit(21, r)]
    partial_blocks_for_write_allowed: bool,
    /// FILE_FORMAT_GROUP field.
    #[bit(15, rw)]
    file_format_group_set: bool,
    /// COPY field.
    #[bit(14, rw)]
    copy_flag_set: bool,
    /// PERM_WRITE_PROTECT field.
    #[bit(13, rw)]
    permanent_write_protection: bool,
    /// TEMP_WRITE_PROTECT field.
    #[bit(12, rw)]
    temporary_write_protection: bool,
    /// FILE_FORMAT field.
    #[bits(10..=11, rw)]
    file_format: u2,
    /// WP_UPC field.
    #[bit(9, rw)]
    write_protection_until_power_cycle: bool,
    /// CRC field - CRC7 checksum.
    #[bits(1..=7, rw)]
    crc: u7,
}

impl CsdV3 {
    /// Create [Self] from a raw byte slice.
    pub fn from_be_bytes(bytes: &[u8; 16]) -> Self {
        Self::new_with_raw_value(u128::from_be_bytes(*bytes))
    }

    /// Returns the card capacity in bytes
    pub fn card_capacity_bytes(&self) -> u64 {
        (u64::from(self.device_size()) + 1) * 512 * 1024
    }

    /// Returns the card capacity in 512-byte blocks
    pub fn card_capacity_blocks(&self) -> u32 {
        (self.device_size().as_u32() + 1) * 1024
    }

    /// Verify CRC7 checksum.
    pub fn verify_crc7(&self) -> bool {
        let raw_bytes = self.raw_value().to_be_bytes();
        let calculated = crc7(&raw_bytes[0..15]);
        self.crc().value() == calculated
    }
}

/// Calculate the 7-bit CRC used on the SD card
pub fn crc7(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for mut d in data.iter().cloned() {
        for _bit in 0..8 {
            crc <<= 1;
            if ((d & 0x80) ^ (crc & 0x80)) != 0 {
                crc ^= 0x09;
            }
            d <<= 1;
        }
    }
    crc & 0x7F
}

/// Perform the X25 CRC calculation, as used for data blocks.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &byte in data {
        crc = ((crc >> 8) & 0xFF) | (crc << 8);
        crc ^= u16::from(byte);
        crc ^= (crc & 0xFF) >> 4;
        crc ^= crc << 12;
        crc ^= (crc & 0xFF) << 5;
    }
    crc
}

// ****************************************************************************
//
// Unit Tests
//
// ****************************************************************************

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_crc7_0() {
        const DATA: [u8; 15] = hex!("00 26 00 32 5F 59 83 C8 AD DB CF FF D2 40 40");
        assert_eq!(crc7(&DATA), 0x52);
    }

    #[test]
    fn test_crc7_1() {
        // Taken from page 119 of the SD card spec.
        let cmd0_arg0: [u8; 5] = [0b01000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000];
        assert_eq!(crc7(&cmd0_arg0), 0b1001010);
    }

    #[test]
    fn test_crc7_2() {
        // Taken from page 119 of the SD card spec.
        let cmd17_arg0: [u8; 5] = [0b01010001, 0b00000000, 0b00000000, 0b00000000, 0b00000000];
        assert_eq!(crc7(&cmd17_arg0), 0b0101010);
    }

    #[test]
    fn test_crc7_3() {
        // Taken from page 119 of the SD card spec.
        let cmd17_response: [u8; 5] = [0b00010001, 0b00000000, 0b00000000, 0b00001001, 0b00000000];
        assert_eq!(crc7(&cmd17_response), 0b0110011);
    }

    #[test]
    fn test_crc16() {
        // An actual CSD read from an SD card
        const DATA: [u8; 16] = hex!("00 26 00 32 5F 5A 83 AE FE FB CF FF 92 80 40 DF");
        assert_eq!(crc16(&DATA), 0x9fc5);
    }

    #[test]
    fn test_csdv1b() {
        const EXAMPLE_HEX: [u8; 16] = hex!("00 26 00 32 5F 59 83 C8 AD DB CF FF D2 40 40 A5");
        const EXAMPLE_U128: u128 = u128::from_be_bytes(EXAMPLE_HEX);
        const EXAMPLE: CsdV1 = CsdV1::new_with_raw_value(EXAMPLE_U128);

        let csd = Csd::new(&EXAMPLE_HEX);
        assert!(csd.is_ok());
        if let Ok(Csd::V1(csd_v1)) = csd {
            assert_eq!(csd_v1.raw_value(), EXAMPLE_U128);
        }

        // CSD Structure: describes version of CSD structure
        // 0b00 [Interpreted: Version 1.0]
        assert_eq!(EXAMPLE.csd_structure().unwrap(), CsdStructure::CsdV1);

        // Data Read Access Time 1: defines Asynchronous part of the read
        // access time 0x26 [Interpreted: 1.5 x 1ms]
        assert_eq!(EXAMPLE.data_read_access_time1(), 0x26);

        // Data Read Access Time 2: worst case clock dependent factor for data
        // access time 0x00 [Decimal: 0 x 100 Clocks]
        assert_eq!(EXAMPLE.data_read_access_time2(), 0x00);

        // Max Data Transfer Rate: sometimes stated as Mhz
        // 0x32 [Interpreted: 2.5 x 10Mbit/s]
        assert_eq!(EXAMPLE.max_data_transfer_rate(), 0x32);

        // Card Command Classes: 0x5f5 [Interpreted: Class 0: Yes. Class 1:
        // No. Class 2: Yes. Class 3: No. Class 4: Yes. Class 5: Yes. Class 6:
        // Yes. Class 7: Yes. Class 8: Yes. Class 9: No. Class 10: Yes. Class
        // 11: No. ]
        assert_eq!(EXAMPLE.card_command_classes_raw().value(), 0x5f5);

        // Max Read Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(
            EXAMPLE.read_block_length().unwrap(),
            BlockLengthSelectV1::_512
        );

        // Partial Blocks for Read Allowed:
        // 0b1 [Interpreted: Yes]
        assert!(EXAMPLE.partial_blocks_for_read_allowed());

        // Write Block Misalignment:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.write_block_misalignment());

        // Read Block Misalignment:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.read_block_misalignment());

        // DSR Implemented: indicates configurable driver stage integrated on
        // card 0b0 [Interpreted: No]
        assert!(!EXAMPLE.dsr_implemented());

        // Device Size: to calculate the card capacity excl. security area
        // ((device size + 1)*device size multiplier*max read data block
        // length) bytes 0xf22 [Decimal: 3874]
        assert_eq!(EXAMPLE.device_size().value(), 3874);

        // Max Read Current @ VDD Min:
        // 0x5 [Interpreted: 35mA]
        assert_eq!(EXAMPLE.max_read_current_vdd_min().value(), 5);

        // Max Read Current @ VDD Max:
        // 0x5 [Interpreted: 80mA]
        assert_eq!(EXAMPLE.max_read_current_vdd_max().value(), 5);

        // Max Write Current @ VDD Min:
        // 0x6 [Interpreted: 60mA]
        assert_eq!(EXAMPLE.max_write_current_vdd_min().value(), 6);

        // Max Write Current @ VDD Max::
        // 0x6 [Interpreted: 200mA]
        assert_eq!(EXAMPLE.max_write_current_vdd_max().value(), 6);

        // Device Size Multiplier:
        // 0x7 [Interpreted: x512]
        assert_eq!(EXAMPLE.device_size_multiplier(), SizeMultiplierSelect::_512);

        // Erase Single Block Enabled:
        // 0x1 [Interpreted: Yes]
        assert!(EXAMPLE.erase_single_block_enabled());

        // Erase Sector Size: size of erasable sector in write blocks
        // 0x1f [Interpreted: 32 blocks]
        assert_eq!(EXAMPLE.erase_sector_size().value(), 0x1F);

        // Write Protect Group Size:
        // 0x7f [Interpreted: 128 sectors]
        assert_eq!(EXAMPLE.write_protect_group_size().value(), 0x7f);

        // Write Protect Group Enable:
        // 0x1 [Interpreted: Yes]
        assert!(EXAMPLE.write_protect_group_enable());

        // Write Speed Factor: block program time as multiple of read access time
        // 0x4 [Interpreted: x16]
        assert_eq!(EXAMPLE.write_speed_factor().value(), 0x4);

        // Max Write Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(
            EXAMPLE.write_block_length().unwrap(),
            BlockLengthSelectV1::_512
        );

        // Partial Blocks for Write Allowed:
        // 0x0 [Interpreted: No]
        assert!(!EXAMPLE.partial_blocks_for_write_allowed());

        // File Format Group:
        // 0b0 [Interpreted: is either Hard Disk with Partition Table/DOS FAT without Partition Table/Universal File Format/Other/Unknown]
        assert!(!EXAMPLE.file_format_group_set());

        // Copy Flag:
        // 0b1 [Interpreted: Non-Original]
        assert!(EXAMPLE.copy_flag_set());

        // Permanent Write Protection:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.permanent_write_protection());

        // Temporary Write Protection:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.temporary_write_protection());

        // File Format:
        // 0x0 [Interpreted: Hard Disk with Partition Table]
        assert_eq!(EXAMPLE.file_format().value(), 0x00);

        // CRC7 Checksum:
        assert_eq!(EXAMPLE.crc().value(), 0x52);

        assert_eq!(EXAMPLE.card_capacity_bytes(), 1_015_808_000);
        assert_eq!(EXAMPLE.card_capacity_blocks(), 1_984_000);

        assert!(EXAMPLE.verify_crc7());
    }

    #[test]
    fn test_csd_invalid_checksum() {
        const EXAMPLE_HEX: [u8; 16] = hex!("00 26 00 32 5F 59 83 C8 AD DB CF FF D2 40 40 FF");
        let csd = Csd::new(&EXAMPLE_HEX);
        assert_eq!(csd.unwrap_err(), CsdCreationError::Checksum)
    }

    #[test]
    fn test_csd_invalid_leading_field() {
        const EXAMPLE_HEX: [u8; 16] = hex!("FF 26 00 32 5F 59 83 C8 AD DB CF FF D2 40 40 A4");
        let csd = Csd::new(&EXAMPLE_HEX);
        assert_eq!(csd.unwrap_err(), CsdCreationError::InvalidCsdStructureField)
    }

    #[test]
    fn test_csdv1() {
        const EXAMPLE: CsdV1 = CsdV1::new_with_raw_value(u128::from_be_bytes(hex!(
            "00 7F 00 32 5B 5A 83 AF 7F FF CF 80 16 80 00 6F"
        )));
        // CSD Structure: describes version of CSD structure
        // 0b00 [Interpreted: Version 1.0]
        assert_eq!(EXAMPLE.csd_structure().unwrap(), CsdStructure::CsdV1);

        // Data Read Access Time 1: defines Asynchronous part of the read access time
        // 0x7f [Interpreted: 8.0 x 10ms]
        assert_eq!(EXAMPLE.data_read_access_time1(), 0x7F);

        // Data Read Access Time 2: worst case clock dependent factor for data access time
        // 0x00 [Decimal: 0 x 100 Clocks]
        assert_eq!(EXAMPLE.data_read_access_time2(), 0x00);

        // Max Data Transfer Rate: sometimes stated as Mhz
        // 0x32 [Interpreted: 2.5 x 10Mbit/s]
        assert_eq!(EXAMPLE.max_data_transfer_rate(), 0x32);

        // Card Command Classes:
        // 0x5b5 [Interpreted: Class 0: Yes. Class 1: No. Class 2: Yes. Class 3: No. Class 4: Yes. Class 5: Yes. Class 6: No. Class 7: Yes. Class 8: Yes. Class 9: No. Class 10: Yes. Class 11: No. ]
        assert_eq!(EXAMPLE.card_command_classes_raw().value(), 0x5b5);
        assert_eq!(
            EXAMPLE.card_command_classes(),
            CardCommandClasses::BASIC
                | CardCommandClasses::BLOCK_READ
                | CardCommandClasses::BLOCK_WRITE
                | CardCommandClasses::ERASE
                | CardCommandClasses::LOCK_CARD
                | CardCommandClasses::APP_SPECIFIC
                | CardCommandClasses::SWITCH
        );

        // Max Read Data Block Length:
        // 0xa [Interpreted: 1024 Bytes]
        assert_eq!(
            EXAMPLE.read_block_length().unwrap(),
            BlockLengthSelectV1::_1024
        );

        // Partial Blocks for Read Allowed:
        // 0b1 [Interpreted: Yes]
        assert!(EXAMPLE.partial_blocks_for_read_allowed());

        // Write Block Misalignment:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.write_block_misalignment());

        // Read Block Misalignment:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.read_block_misalignment());

        // DSR Implemented: indicates configurable driver stage integrated on card
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.dsr_implemented());

        // Device Size: to calculate the card capacity excl. security area
        // ((device size + 1)*device size multiplier*max read data block
        // length) bytes 0xebd [Decimal: 3773]
        assert_eq!(EXAMPLE.device_size().value(), 3773);

        // Max Read Current @ VDD Min:
        // 0x7 [Interpreted: 100mA]
        assert_eq!(EXAMPLE.max_read_current_vdd_min().value(), 7);

        // Max Read Current @ VDD Max:
        // 0x7 [Interpreted: 200mA]
        assert_eq!(EXAMPLE.max_read_current_vdd_max().value(), 7);

        // Max Write Current @ VDD Min:
        // 0x7 [Interpreted: 100mA]
        assert_eq!(EXAMPLE.max_write_current_vdd_min().value(), 7);

        // Max Write Current @ VDD Max::
        // 0x7 [Interpreted: 200mA]
        assert_eq!(EXAMPLE.max_write_current_vdd_max().value(), 7);

        // Device Size Multiplier:
        // 0x7 [Interpreted: x512]
        assert_eq!(EXAMPLE.device_size_multiplier(), SizeMultiplierSelect::_512);

        // Erase Single Block Enabled:
        // 0x1 [Interpreted: Yes]
        assert!(EXAMPLE.erase_single_block_enabled());

        // Erase Sector Size: size of erasable sector in write blocks
        // 0x1f [Interpreted: 32 blocks]
        assert_eq!(EXAMPLE.erase_sector_size().value(), 0x1F);

        // Write Protect Group Size:
        // 0x00 [Interpreted: 1 sectors]
        assert_eq!(EXAMPLE.write_protect_group_size().value(), 0x00);

        // Write Protect Group Enable:
        // 0x0 [Interpreted: No]
        assert!(!EXAMPLE.write_protect_group_enable());

        // Write Speed Factor: block program time as multiple of read access time
        // 0x5 [Interpreted: x32]
        assert_eq!(EXAMPLE.write_speed_factor().value(), 0x5);

        // Max Write Data Block Length:
        // 0xa [Interpreted: 1024 Bytes]
        assert_eq!(
            EXAMPLE.write_block_length().unwrap(),
            BlockLengthSelectV1::_1024
        );

        // Partial Blocks for Write Allowed:
        // 0x0 [Interpreted: No]
        assert!(!EXAMPLE.partial_blocks_for_write_allowed());

        // File Format Group:
        // 0b0 [Interpreted: is either Hard Disk with Partition Table/DOS FAT without Partition Table/Universal File Format/Other/Unknown]
        assert!(!EXAMPLE.file_format_group_set());

        // Copy Flag:
        // 0b0 [Interpreted: Original]
        assert!(!EXAMPLE.copy_flag_set());

        // Permanent Write Protection:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.permanent_write_protection());

        // Temporary Write Protection:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.temporary_write_protection());

        // File Format:
        // 0x0 [Interpreted: Hard Disk with Partition Table]
        assert_eq!(EXAMPLE.file_format().value(), 0x00);

        // CRC7 Checksum.
        assert_eq!(EXAMPLE.crc().value(), 0x37);

        assert_eq!(EXAMPLE.card_capacity_bytes(), 1_978_662_912);
        assert_eq!(EXAMPLE.card_capacity_blocks(), 3_864_576);
    }

    #[test]
    fn test_csdv2() {
        const EXAMPLE_HEX: [u8; 16] = hex!("40 0E 00 32 5B 59 00 00 1D 69 7F 80 0A 40 00 8B");
        const EXAMPLE_U128: u128 = u128::from_be_bytes(EXAMPLE_HEX);
        const EXAMPLE: CsdV2 = CsdV2::new_with_raw_value(EXAMPLE_U128);

        let csd = Csd::new(&EXAMPLE_HEX);
        assert!(csd.is_ok());
        if let Ok(Csd::V2(csd_v2)) = csd {
            assert_eq!(csd_v2.raw_value(), EXAMPLE_U128);
        }

        // CSD Structure: describes version of CSD structure
        // 0b01 [Interpreted: Version 2.0 SDHC]
        assert_eq!(EXAMPLE.csd_structure().unwrap(), CsdStructure::CsdV2);

        // Data Read Access Time 1: defines Asynchronous part of the read access time
        // 0x0e [Interpreted: 1.0 x 1ms]
        assert_eq!(EXAMPLE.data_read_access_time1(), 0x0E);

        // Data Read Access Time 2: worst case clock dependent factor for data access time
        // 0x00 [Decimal: 0 x 100 Clocks]
        assert_eq!(EXAMPLE.data_read_access_time2(), 0x00);

        // Max Data Transfer Rate: sometimes stated as Mhz
        // 0x32 [Interpreted: 2.5 x 10Mbit/s]
        assert_eq!(EXAMPLE.max_data_transfer_rate(), 0x32);

        // Card Command Classes:
        // 0x5b5 [Interpreted: Class 0: Yes. Class 1: No. Class 2: Yes. Class 3: No. Class 4: Yes. Class 5: Yes. Class 6: No. Class 7: Yes. Class 8: Yes. Class 9: No. Class 10: Yes. Class 11: No. ]
        assert_eq!(EXAMPLE.card_command_classes_raw().value(), 0x5b5);

        // Max Read Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(
            EXAMPLE.read_block_length().unwrap(),
            BlockLengthSelectV2AndV3::_512
        );

        // Partial Blocks for Read Allowed:
        // 0b0 [Interpreted: Yes]
        assert!(!EXAMPLE.partial_blocks_for_read_allowed());

        // Write Block Misalignment:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.write_block_misalignment());

        // Read Block Misalignment:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.read_block_misalignment());

        // DSR Implemented: indicates configurable driver stage integrated on card
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.dsr_implemented());

        // Device Size: to calculate the card capacity excl. security area
        // ((device size + 1)* 512kbytes
        // 0x001d69 [Decimal: 7529]
        assert_eq!(EXAMPLE.device_size().value(), 7529);

        // Erase Single Block Enabled:
        // 0x1 [Interpreted: Yes]
        assert!(EXAMPLE.erase_single_block_enabled());

        // Erase Sector Size: size of erasable sector in write blocks
        // 0x7f [Interpreted: 128 blocks]
        assert_eq!(EXAMPLE.erase_sector_size().value(), 0x7F);

        // Write Protect Group Size:
        // 0x00 [Interpreted: 1 sectors]
        assert_eq!(EXAMPLE.write_protect_group_size().value(), 0x00);

        // Write Protect Group Enable:
        // 0x0 [Interpreted: No]
        assert!(!EXAMPLE.write_protect_group_enable());

        // Write Speed Factor: block program time as multiple of read access time
        // 0x2 [Interpreted: x4]
        assert_eq!(EXAMPLE.write_speed_factor().value(), 0x2);

        // Max Write Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(
            EXAMPLE.write_block_length().unwrap(),
            BlockLengthSelectV2AndV3::_512
        );

        // Partial Blocks for Write Allowed:
        // 0x0 [Interpreted: No]
        assert!(!EXAMPLE.partial_blocks_for_write_allowed());

        // File Format Group:
        // 0b0 [Interpreted: is either Hard Disk with Partition Table/DOS FAT without Partition Table/Universal File Format/Other/Unknown]
        assert!(!EXAMPLE.file_format_group_set());

        // Copy Flag:
        // 0b0 [Interpreted: Original]
        assert!(!EXAMPLE.copy_flag_set());

        // Permanent Write Protection:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.permanent_write_protection());

        // Temporary Write Protection:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.temporary_write_protection());

        // File Format:
        // 0x0 [Interpreted: Hard Disk with Partition Table]
        assert_eq!(EXAMPLE.file_format().value(), 0x00);

        // CRC7 Checksum.
        assert_eq!(EXAMPLE.crc().value(), 0x45);

        assert_eq!(EXAMPLE.card_capacity_bytes(), 3_947_888_640);
        assert_eq!(EXAMPLE.card_capacity_blocks(), 7_710_720);

        assert!(EXAMPLE.verify_crc7());
    }

    #[test]
    fn test_csdv2b() {
        const EXAMPLE: CsdV2 = CsdV2::new_with_raw_value(u128::from_be_bytes(hex!(
            "40 0E 00 32 5B 59 00 00 3A 91 7F 80 0A 40 00 05"
        )));
        // CSD Structure: describes version of CSD structure
        // 0b01 [Interpreted: Version 2.0 SDHC]
        assert_eq!(EXAMPLE.csd_structure().unwrap(), CsdStructure::CsdV2);

        // Data Read Access Time 1: defines Asynchronous part of the read access time
        // 0x0e [Interpreted: 1.0 x 1ms]
        assert_eq!(EXAMPLE.data_read_access_time1(), 0x0E);

        // Data Read Access Time 2: worst case clock dependent factor for data access time
        // 0x00 [Decimal: 0 x 100 Clocks]
        assert_eq!(EXAMPLE.data_read_access_time2(), 0x00);

        // Max Data Transfer Rate: sometimes stated as Mhz
        // 0x32 [Interpreted: 2.5 x 10Mbit/s]
        assert_eq!(EXAMPLE.max_data_transfer_rate(), 0x32);

        // Card Command Classes:
        // 0x5b5 [Interpreted: Class 0: Yes. Class 1: No. Class 2: Yes. Class 3: No. Class 4: Yes. Class 5: Yes. Class 6: No. Class 7: Yes. Class 8: Yes. Class 9: No. Class 10: Yes. Class 11: No. ]
        assert_eq!(EXAMPLE.card_command_classes_raw().value(), 0x5b5);

        // Max Read Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(
            EXAMPLE.read_block_length().unwrap(),
            BlockLengthSelectV2AndV3::_512
        );

        // Partial Blocks for Read Allowed:
        // 0b0 [Interpreted: Yes]
        assert!(!EXAMPLE.partial_blocks_for_read_allowed());

        // Write Block Misalignment:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.write_block_misalignment());

        // Read Block Misalignment:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.read_block_misalignment());

        // DSR Implemented: indicates configurable driver stage integrated on card
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.dsr_implemented());

        // Device Size: to calculate the card capacity excl. security area
        // ((device size + 1)* 512kbytes
        // 0x003a91 [Decimal: 7529]
        assert_eq!(EXAMPLE.device_size().value(), 14993);

        // Erase Single Block Enabled:
        // 0x1 [Interpreted: Yes]
        assert!(EXAMPLE.erase_single_block_enabled());

        // Erase Sector Size: size of erasable sector in write blocks
        // 0x7f [Interpreted: 128 blocks]
        assert_eq!(EXAMPLE.erase_sector_size().value(), 0x7F);

        // Write Protect Group Size:
        // 0x00 [Interpreted: 1 sectors]
        assert_eq!(EXAMPLE.write_protect_group_size().value(), 0x00);

        // Write Protect Group Enable:
        // 0x0 [Interpreted: No]
        assert!(!EXAMPLE.write_protect_group_enable());

        // Write Speed Factor: block program time as multiple of read access time
        // 0x2 [Interpreted: x4]
        assert_eq!(EXAMPLE.write_speed_factor().value(), 0x2);

        // Max Write Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(
            EXAMPLE.write_block_length().unwrap(),
            BlockLengthSelectV2AndV3::_512
        );

        // Partial Blocks for Write Allowed:
        // 0x0 [Interpreted: No]
        assert!(!EXAMPLE.partial_blocks_for_write_allowed());

        // File Format Group:
        // 0b0 [Interpreted: is either Hard Disk with Partition Table/DOS FAT without Partition Table/Universal File Format/Other/Unknown]
        assert!(!EXAMPLE.file_format_group_set());

        // Copy Flag:
        // 0b0 [Interpreted: Original]
        assert!(!EXAMPLE.copy_flag_set());

        // Permanent Write Protection:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.permanent_write_protection());

        // Temporary Write Protection:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.temporary_write_protection());

        // File Format:
        // 0x0 [Interpreted: Hard Disk with Partition Table]
        assert_eq!(EXAMPLE.file_format().value(), 0x00);

        // CRC7 Checksum.
        assert_eq!(EXAMPLE.crc().value(), 0x02);

        assert_eq!(EXAMPLE.card_capacity_bytes(), 7_861_174_272);
        assert_eq!(EXAMPLE.card_capacity_blocks(), 15_353_856);

        assert!(EXAMPLE.verify_crc7());
    }

    #[test]
    fn test_csdv2c() {
        const EXAMPLE: CsdV2 = CsdV2::new_with_raw_value(u128::from_be_bytes(hex!(
            "40 0e 00 32 5b 59 00 00 3a e3 7f 80 0a 40 00 57"
        )));

        // CSD Structure: describes version of CSD structure
        // 0b01 [Interpreted: Version 2.0 SDHC]
        assert_eq!(EXAMPLE.csd_structure().unwrap(), CsdStructure::CsdV2);

        // Data Read Access Time 1: defines Asynchronous part of the read access time
        // 0x0e [Interpreted: 1.0 x 1ms]
        assert_eq!(EXAMPLE.data_read_access_time1(), 0x0E);

        // Data Read Access Time 2: worst case clock dependent factor for data access time
        // 0x00 [Decimal: 0 x 100 Clocks]
        assert_eq!(EXAMPLE.data_read_access_time2(), 0x00);

        // Max Data Transfer Rate: sometimes stated as Mhz
        // 0x32 [Interpreted: 2.5 x 10Mbit/s]
        assert_eq!(EXAMPLE.max_data_transfer_rate(), 0x32);

        // Card Command Classes:
        // 0x5b5 [Interpreted: Class 0: Yes. Class 1: No. Class 2: Yes. Class 3: No. Class 4: Yes. Class 5: Yes. Class 6: No. Class 7: Yes. Class 8: Yes. Class 9: No. Class 10: Yes. Class 11: No. ]
        assert_eq!(EXAMPLE.card_command_classes_raw().value(), 0x5b5);

        // Max Read Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(
            EXAMPLE.read_block_length().unwrap(),
            BlockLengthSelectV2AndV3::_512
        );

        // Partial Blocks for Read Allowed:
        // 0b0 [Interpreted: Yes]
        assert!(!EXAMPLE.partial_blocks_for_read_allowed());

        // Write Block Misalignment:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.write_block_misalignment());

        // Read Block Misalignment:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.read_block_misalignment());

        // DSR Implemented: indicates configurable driver stage integrated on card
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.dsr_implemented());

        // Device Size: to calculate the card capacity excl. security area
        // ((device size + 1)* 512kbytes
        // 0x003a91 [Decimal: 7529]
        assert_eq!(EXAMPLE.device_size().value(), 15075);

        // Erase Single Block Enabled:
        // 0x1 [Interpreted: Yes]
        assert!(EXAMPLE.erase_single_block_enabled());

        // Erase Sector Size: size of erasable sector in write blocks
        // 0x7f [Interpreted: 128 blocks]
        assert_eq!(EXAMPLE.erase_sector_size().value(), 0x7F);

        // Write Protect Group Size:
        // 0x00 [Interpreted: 1 sectors]
        assert_eq!(EXAMPLE.write_protect_group_size().value(), 0x00);

        // Write Protect Group Enable:
        // 0x0 [Interpreted: No]
        assert!(!EXAMPLE.write_protect_group_enable());

        // Write Speed Factor: block program time as multiple of read access time
        // 0x2 [Interpreted: x4]
        assert_eq!(EXAMPLE.write_speed_factor().value(), 0x2);

        // Max Write Data Block Length:
        // 0x9 [Interpreted: 512 Bytes]
        assert_eq!(
            EXAMPLE.write_block_length().unwrap(),
            BlockLengthSelectV2AndV3::_512
        );

        // Partial Blocks for Write Allowed:
        // 0x0 [Interpreted: No]
        assert!(!EXAMPLE.partial_blocks_for_write_allowed());

        // File Format Group:
        // 0b0 [Interpreted: is either Hard Disk with Partition Table/DOS FAT without Partition Table/Universal File Format/Other/Unknown]
        assert!(!EXAMPLE.file_format_group_set());

        // Copy Flag:
        // 0b0 [Interpreted: Original]
        assert!(!EXAMPLE.copy_flag_set());

        // Permanent Write Protection:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.permanent_write_protection());

        // Temporary Write Protection:
        // 0b0 [Interpreted: No]
        assert!(!EXAMPLE.temporary_write_protection());

        // File Format:
        // 0x0 [Interpreted: Hard Disk with Partition Table]
        assert_eq!(EXAMPLE.file_format().value(), 0x00);

        // CRC7 Checksum.
        assert_eq!(EXAMPLE.crc().value(), 0x2B);

        // 8 GB.
        assert_eq!(EXAMPLE.card_capacity_bytes(), 7_904_165_888);
        assert_eq!(EXAMPLE.card_capacity_blocks(), 15_437_824);

        assert!(EXAMPLE.verify_crc7());
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
