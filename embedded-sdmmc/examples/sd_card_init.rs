//! This example shows how to a SD card implementation could look like uisng a virtual SD card
//! implementation provided by the [embedded_sdmmc_types] crate.
//!
//! It should be noted that you only have to depend on [embedded_sdmmc_types] to add
//! [embedded_sdmmc] support to your project.
use anyhow::{Context as _, bail};
use embedded_sdmmc_types::sdcard::argument::{
    Acmd6, Acmd41, Cmd7, Cmd8, Cmd9, Cmd13, OcrLower, VoltageSuppliedSelect,
};
use embedded_sdmmc_types::sdcard::mock::SdCardMock;
use embedded_sdmmc_types::sdcard::response::{self, R1, R3, R6, R7};
use embedded_sdmmc_types::sdcard::{self, AcmdId, CardType, CmdId};
use embedded_sdmmc_types::sdcard::{cid::Cid, csd::Csd};
use embedded_sdmmc_types::{Block, BlockCount, BlockDevice, BlockIdx};

/// Negotiated as part of ACMD41 during SD card initialization.
pub const VOLTAGE_LEVEL_CAPABILITIES: OcrLower = OcrLower::builder()
    .with__3_5_to_3_6v(false)
    .with__3_4_to_3_5v(false)
    .with__3_3_to_3_4v(false)
    .with__3_2_to_3_3v(true)
    .with__3_1_to_3_2v(false)
    .with__3_0_to_3_1v(false)
    .with__2_9_to_3_0v(false)
    .with__2_8_to_2_9v(false)
    .with__2_7_to_2_8v(false)
    .with_reserved_low_voltage(false)
    .build();

pub struct SdCardUninit(SdCardMock);

impl SdCardUninit {
    pub fn new(sd_mock: SdCardMock) -> Self {
        Self(sd_mock)
    }

    /// Example initialization sequence for a SD card which can provide
    /// a reference example for a real driver.
    ///
    /// It performs the initialization sequence specified in the SD card
    /// physical layer specification Ver9.10, p.68 and p.70.
    pub fn initialize(mut self) -> Result<SdCard, anyhow::Error> {
        self.0.insert_command(CmdId::CMD0_GoIdleState, 0);

        // Voltage level negotiation. Send CMD8 first.
        let status = self.0.insert_command(
            CmdId::CMD8_SendIfCond,
            Cmd8::ZERO
                .with_voltage_supplied(VoltageSuppliedSelect::_2_7To3_6V)
                .with_check_pattern(0xAA)
                .raw_value(),
        );
        let responded_to_cmd8 = !status.timeout();

        let hcs = if responded_to_cmd8 {
            let r7 = R7::new_with_raw_value(self.0.read_reply_u32());
            if r7
                .voltage_accepted()
                .is_ok_and(|val| val != VoltageSuppliedSelect::_2_7To3_6V)
            {
                bail!("CMD8 reply R7: Voltage not accepted");
            }
            if r7.echo_check_pattern() != 0xAA {
                bail!("CMD8 reply R7: Check pattern missmatch");
            }
            sdcard::argument::HostCapacitySupport::SdhcOrSdxc
        } else {
            sdcard::argument::HostCapacitySupport::SdscOnly
        };

        // Now send ACMD41.
        self.0.insert_acmd(
            AcmdId::ACMD41_SdSendOpCond,
            Acmd41::builder()
                .with_host_capacity_support(hcs)
                .with_fast_boot(false)
                .with_xpc(sdcard::argument::PowerControl::MaximumPerformance)
                .with_s18r(false)
                .with_ocr(VOLTAGE_LEVEL_CAPABILITIES)
                .build()
                .raw_value(),
        );
        let mut r3;
        loop {
            // Now poll until the card initialization is complete. In real driver code, timeout
            // handling or an upper polling limit might be a good idea.
            self.0.insert_acmd(AcmdId::ACMD41_SdSendOpCond, 0);
            r3 = R3::new_with_raw_value(self.0.read_reply_u32());
            if r3.initialization_complete() {
                break;
            }
        }

        let card_type = if responded_to_cmd8 {
            if r3.card_capacity_status() {
                CardType::SdhcSdxc
            } else {
                CardType::SD2
            }
        } else {
            CardType::SD1
        };

        // Retrieve and cache the CID. This puts it into identification mode.
        self.0.insert_command(CmdId::CMD2_AllSendCid, 0);
        let cid_raw = self.0.read_reply_u128();
        let cid = sdcard::cid::Cid::new_with_raw_value(cid_raw);

        // Send CMD3 to retrieve RCA required for card addressing, as well as put the card
        // into standby mode.
        self.0.insert_command(CmdId::CMD3_SendRelativeAddr, 0);
        let r6 = R6::new_with_raw_value(self.0.read_reply_u32());
        let rca = r6.rca();

        // Retrieve and cache CSD, which also contains card specific data.
        self.0
            .insert_command(CmdId::CMD9_SendCsd, Cmd9::ZERO.with_rca(rca).raw_value());
        let cid_raw = self.0.read_reply_u128();
        let csd = Csd::new(&cid_raw.to_be_bytes()).with_context(|| "failed to parse CSD")?;

        // CMD7 to put the SD card into transfer state.
        self.0
            .insert_command(CmdId::CMD7_SelectCard, Cmd7::ZERO.with_rca(rca).raw_value());

        // Check that the card is in transfer mode.
        self.0.insert_command(
            CmdId::CMD13_SendStatus,
            Cmd13::ZERO.with_rca(rca).raw_value(),
        );
        let r1 = R1::new_with_raw_value(self.0.read_reply_u32());

        if r1.state().is_ok_and(|state| state != response::State::Tran) {
            bail!("CMD8 reply R1: Not in transfer state");
        }

        // In this example, we put the SD card into 4-bit mode. On real hardware, you usually
        // have to configure register bits on the controller side as well.
        self.0.insert_acmd(
            AcmdId::ACMD6_SetBusWidth,
            Acmd6::builder()
                .with_bus_width(sdcard::argument::BusWidth::_4bits)
                .build()
                .raw_value(),
        );

        Ok(SdCard {
            card_type,
            cid,
            csd,
            rca,
            sd_mock: core::cell::RefCell::new(self.0),
        })
    }
}

#[derive(Debug)]
pub struct SdCard {
    card_type: CardType,
    cid: Cid,
    csd: Csd,
    rca: u16,
    #[allow(unused)]
    sd_mock: core::cell::RefCell<SdCardMock>,
}

#[derive(thiserror::Error, Debug)]
pub enum TransferError {
    #[error("SD card is not in transfer state")]
    InvalidState,
    #[error("invalid buffer size")]
    InvalidBufferSize,
}

pub const BLOCK_LEN: usize = 512;

impl SdCard {
    pub fn read_single_block(&self, buf: &mut [u8], addr: u32) -> Result<(), TransferError> {
        if self.sd_mock.borrow().read_current_state() != sdcard::mock::State::Tran {
            return Err(TransferError::InvalidState);
        }
        if buf.len() != BLOCK_LEN {
            return Err(TransferError::InvalidBufferSize);
        }
        let mut sd_mock = self.sd_mock.borrow_mut();
        sd_mock.insert_command(CmdId::CMD17_ReadSingleBlock, addr);
        sd_mock.wait_until_data_transfer_done();
        let mut bytes_read = 0;
        while bytes_read < BLOCK_LEN {
            let word = sd_mock.read_data_word();
            buf[bytes_read..bytes_read + 4].copy_from_slice(&word.to_ne_bytes());
            bytes_read += 4;
        }
        Ok(())
    }

    pub fn write_single_block(&self, buf: &[u8], addr: u32) -> Result<(), TransferError> {
        if self.sd_mock.borrow().read_current_state() != sdcard::mock::State::Tran {
            return Err(TransferError::InvalidState);
        }
        if buf.len() != BLOCK_LEN {
            return Err(TransferError::InvalidBufferSize);
        }
        let mut sd_mock = self.sd_mock.borrow_mut();
        sd_mock.insert_command(CmdId::CMD24_WriteBlock, addr);
        let mut bytes_written = 0;
        while bytes_written < BLOCK_LEN {
            sd_mock.write_data_word(u32::from_ne_bytes(
                buf[bytes_written..bytes_written + 4].try_into().unwrap(),
            ));
            bytes_written += 4;
        }
        sd_mock.wait_until_data_transfer_done();

        // On some SD card, even after the data transfer is done, you might need to wait until the
        // SD card goes from the programming state back to the transfer state.
        while sd_mock.read_current_state() != sdcard::mock::State::Tran {}
        Ok(())
    }
}

impl BlockDevice for SdCard {
    type Error = TransferError;

    fn read(&self, blocks: &mut [Block], block_idx: BlockIdx) -> Result<(), Self::Error> {
        let addr = match self.card_type {
            CardType::SD1 | CardType::SD2 => block_idx.0 * BLOCK_LEN as u32,
            CardType::SdhcSdxc => block_idx.0,
        };
        for block in blocks.iter_mut() {
            self.read_single_block(block.as_mut_slice(), addr)?;
        }
        Ok(())
    }

    fn write(&self, blocks: &[Block], block_idx: BlockIdx) -> Result<(), Self::Error> {
        let addr = match self.card_type {
            CardType::SD1 | CardType::SD2 => block_idx.0 * BLOCK_LEN as u32,
            CardType::SdhcSdxc => block_idx.0,
        };
        for block in blocks.iter() {
            self.write_single_block(block.as_slice(), addr)?;
        }
        Ok(())
    }

    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        Ok(embedded_sdmmc::BlockCount(self.csd.card_capacity_blocks()))
    }
}

const MOCK_SD_RCA: u16 = 1;

fn main() -> Result<(), anyhow::Error> {
    let sd_mock = SdCardMock::new(CardType::SdhcSdxc, MOCK_SD_RCA);
    let sd_card_uninit = SdCardUninit::new(sd_mock);
    let sd_card = sd_card_uninit
        .initialize()
        .context("failed to initialize SD card")?;
    println!("SD card initialized successfully",);
    println!("--------");
    println!("Card Type: {:?}", sd_card.card_type);
    println!("--------");
    println!("Relative Card Address: {}", sd_card.rca);
    println!("--------");
    println!("CSD: {:?}", sd_card.csd);
    println!("--------");
    println!("CID: {:?}", sd_card.cid);

    // The intialized SD card structure can now be used to write or read blocks.
    Ok(())
}
