//! # Low level SD card access module
//!
//! Contains constants from the SD Specifications.
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

pub mod cid;
pub mod csd;
pub mod spi;

/// The different types of card we support.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CardType {
    /// An standard-capacity SD Card supporting v1.x of the standard.
    ///
    /// Uses byte-addressing internally, so limited to 2GiB in size.
    SD1,
    /// An standard-capacity SD Card supporting v2.x of the standard.
    ///
    /// Uses byte-addressing internally, so limited to 2GiB in size.
    SD2,
    /// An high-capacity 'SDHC' Card or an extended-capacity 'SDXC' card.
    ///
    /// Uses block-addressing internally to support capacities above 2GiB.
    SdhcSdxc,
}

// Possible errors the SD card can return
/// Card indicates last operation was a success
pub const ERROR_OK: u8 = 0x00;

/// Raw command IDs.
#[bitbybit::bitenum(u6, exhaustive = false)]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[allow(non_camel_case_types)]
#[repr(u8)]
pub enum CmdId {
    /// GO_IDLE_STATE - Send cards to IDLE state
    CMD0_GoIdleState = 0,
    /// ALL_SEND_CID - Request Card IDentification (CID)
    CMD2_AllSendCid = 2,
    /// SEND_RELATIVE_ADDR - Request relative card address (RCA)
    CMD3_SendRelativeAddr = 3,
    /// SEND_IF_COND - verify SD Memory Card interface operating condition
    CMD8_SendIfCond = 8,
    /// SELECT/DESELECT_CARD - Select the active card or deselect the active card
    CMD7_SelectCard = 7,
    /// SEND_CSD - read the Card Specific Data (CSD register)
    CMD9_SendCsd = 9,
    /// STOP_TRANSMISSION - Stop a multiple read or write transfer
    CMD12_StopTransmission = 12,
    /// SEND_STATUS / SEND_TASK_STATUS - Read card status register.
    CMD13_SendStatus = 13,
    /// READ_SINGLE_BLOCK - read a single data block from the card
    CMD17_ReadSingleBlock = 17,
    /// READ_MULTIPLE_BLOCK - read a multiple data blocks from the card
    CMD18_ReadMultipleBlock = 18,
    /// WRITE_BLOCK - write a single data block to the card
    CMD24_WriteBlock = 24,
    /// WRITE_MULTIPLE_BLOCK - write blocks of data until a STOP_TRANSMISSION
    CMD25_WriteMultipleBlock = 25,
    /// APP_CMD - escape for application specific command
    CMD55_AppCmd = 55,
    /// READ_OCR - read the OCR register of a card
    CMD58_ReadOcr = 58,
    /// CRC_ON_OFF - enable or disable CRC checking
    CMD59_CrcOnOff = 59,
}

/// Raw application specific IDs ACMD.
#[bitbybit::bitenum(u6, exhaustive = false)]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[allow(non_camel_case_types)]
pub enum AcmdId {
    /// SET_BUS_WIDTH
    ACMD6_SetBusWidth = 6,
    /// SET_WR_BLK_ERASE_COUNT. Pre-erased before writing
    ///
    /// > It is recommended using this command preceding CMD25, some of the cards will be faster for Multiple
    /// > Write Blocks operation. Note that the host should send ACMD23 just before WRITE command if the host
    /// > wants to use the pre-erased feature
    ACMD23_PreErase = 23,
    /// SD_SEND_OP_COND - Sends host capacity support information and activates
    /// the card's initialization process
    ACMD41_SdSendOpCond = 41,
}

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
    use crate::sdcard::csd::*;

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

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
