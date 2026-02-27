//! # CSD (Card Specific Data) module

use arbitrary_int::{traits::Integer as _, u2, u3, u7, u12, u22, u28};

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
#[derive(Debug, PartialEq, Eq, Copy, Clone, thiserror::Error)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
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

    /// Returns the card capacity in 512-byte blocks
    #[inline]
    pub fn card_capacity_blocks(&self) -> u32 {
        match self {
            Csd::V1(csd_v1) => csd_v1.card_capacity_blocks(),
            Csd::V2(csd_v2) => csd_v2.card_capacity_blocks(),
            Csd::V3(csd_v3) => csd_v3.card_capacity_blocks(),
        }
    }

    /// Returns the card capacity in bytes blocks
    #[inline]
    pub fn card_capacity_bytes(&self) -> u64 {
        match self {
            Csd::V1(csd_v1) => csd_v1.card_capacity_bytes(),
            Csd::V2(csd_v2) => csd_v2.card_capacity_bytes(),
            Csd::V3(csd_v3) => csd_v3.card_capacity_bytes(),
        }
    }

    /// Can this card erase single blocks?
    #[inline]
    pub fn erase_single_block_enabled(&self) -> bool {
        match self {
            Csd::V1(csd_v1) => csd_v1.erase_single_block_enabled(),
            Csd::V2(csd_v2) => csd_v2.erase_single_block_enabled(),
            Csd::V3(csd_v3) => csd_v3.erase_single_block_enabled(),
        }
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
#[bitbybit::bitfield(u128, debug, defmt_fields(feature = "defmt-log"), forbid_overlaps)]
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
    #[bit(77, r)]
    read_block_misalignment: bool,
    /// DSR_IMP field.
    #[bit(76, r)]
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
        let calculated = super::crc7(&raw_bytes[0..15]);
        self.crc().value() == calculated
    }
}

/// CSD V2 register structure.
#[bitbybit::bitfield(u128, debug, defmt_fields(feature = "defmt-log"), forbid_overlaps)]
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
    #[bit(77, r)]
    read_block_misalignment: bool,
    /// DSR_IMP field.
    #[bit(76, r)]
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
        let calculated = super::crc7(&raw_bytes[0..15]);
        self.crc().value() == calculated
    }
}

/// CSD V3 register structure.
#[bitbybit::bitfield(u128, debug, defmt_fields(feature = "defmt-log"), forbid_overlaps)]
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
    #[bit(77, r)]
    read_block_misalignment: bool,
    /// DSR_IMP field.
    #[bit(76, r)]
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
        let calculated = super::crc7(&raw_bytes[0..15]);
        self.crc().value() == calculated
    }
}
