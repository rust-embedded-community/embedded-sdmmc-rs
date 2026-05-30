//! # CSD (Card Specific Data) module

use arbitrary_int::{traits::Integer as _, u2, u3, u7, u12, u22, u28};

/// Card Specific Data
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
pub enum Csd {
    /// A version 1 CSD
    V1(CsdV1),
    /// A version 2 CSD
    V2(CsdV2),
    /// A version 3 CSD
    V3(CsdV3),
}

/// The CSD structure, which is the first 2 bits in the raw CSD field, is invalid.
#[derive(Debug, PartialEq, Eq, Copy, Clone, thiserror::Error)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[error("invalid CSD structure field")]
pub struct InvalidCsdStructureFieldError;

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

impl From<InvalidCsdStructureFieldError> for CsdCreationError {
    fn from(_value: InvalidCsdStructureFieldError) -> Self {
        Self::InvalidCsdStructureField
    }
}

impl Csd {
    /// Construct a [Csd] from a raw byte slice with 16 bytes.
    pub fn new(raw: &[u8; 16]) -> Result<Csd, CsdCreationError> {
        let unchecked = Self::new_unchecked(raw)?;
        if !unchecked.verify_crc7() {
            return Err(CsdCreationError::Checksum);
        }
        Ok(unchecked)
    }

    /// Construct a [Csd] without verifying the checksum.
    pub fn new_unchecked(raw: &[u8; 16]) -> Result<Self, InvalidCsdStructureFieldError> {
        let csd_structure_raw = (raw[0] >> 6) & 0b11;

        let csd = match csd_structure_raw {
            0 => Csd::V1(CsdV1::from_be_bytes(raw)),
            1 => Csd::V2(CsdV2::from_be_bytes(raw)),
            2 => Csd::V3(CsdV3::from_be_bytes(raw)),
            _ => return Err(InvalidCsdStructureFieldError),
        };

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
        let block_len = self
            .read_block_length()
            .map_or_else(|err| err.as_u32(), |v| v as u32);
        let sum = (self.device_size_multiplier() as u32).saturating_add(block_len);
        let multiplier = sum.saturating_sub(7);
        if multiplier >= 32 {
            0
        } else {
            (self.device_size().as_u32() + 1)
                .checked_shl(multiplier)
                .unwrap_or(0)
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csdv1b() {
        const EXAMPLE_HEX: [u8; 16] = hex!("00 26 00 32 5F 59 83 C8 AD DB CF FF D2 40 40 A5");
        const EXAMPLE_U128: u128 = u128::from_be_bytes(EXAMPLE_HEX);
        const EXAMPLE: CsdV1 = CsdV1::new_with_raw_value(EXAMPLE_U128);

        let csd = Csd::new(&EXAMPLE_HEX);
        assert!(csd.is_ok());
        if let Ok(Csd::V1(csd_v1)) = csd {
            assert_eq!(csd_v1.raw_value(), EXAMPLE_U128);
        } else {
            panic!("unexpected CSD version, not V1");
        }
        let mut data_without_checksum = EXAMPLE_HEX;
        data_without_checksum[15] = 0;
        let csd_unchecked =
            Csd::new_unchecked(&data_without_checksum).expect("CSD creation failed");
        if let Csd::V1(csd_v1) = csd_unchecked {
            // Ignore the checksum.
            assert_eq!(csd_v1.raw_value() >> 8, EXAMPLE_U128 >> 8);
        } else {
            panic!("unexpected CSD version, not V1");
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
        assert!(matches!(
            csd.unwrap_err(),
            CsdCreationError::InvalidCsdStructureField { .. }
        ));
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
