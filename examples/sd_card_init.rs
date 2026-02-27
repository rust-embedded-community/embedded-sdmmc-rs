use anyhow::{Context as _, bail};
use embedded_sdmmc::sdcard::argument::{Acmd6, Cmd7, Cmd9, Cmd13, OcrLower, VoltageSuppliedSelect};
use embedded_sdmmc::sdcard::mock::SdCardMock;
use embedded_sdmmc::sdcard::response::{self, R1, R3, R6, R7};
use embedded_sdmmc::sdcard::{AcmdId, CmdId, argument};
use embedded_sdmmc::sdcard::{cid::Cid, csd::Csd};

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

pub struct SdCardUninitialized(SdCardMock);

impl SdCardUninitialized {
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
        self.0.insert_command(
            CmdId::CMD8_SendIfCond,
            argument::Cmd8::ZERO
                .with_voltage_supplied(
                    embedded_sdmmc::sdcard::argument::VoltageSuppliedSelect::_2_7To3_6V,
                )
                .with_check_pattern(0xAA)
                .raw_value(),
        );

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

        // Now send ACMD41.
        self.0.insert_acmd(
            AcmdId::ACMD41_SdSendOpCond,
            argument::Acmd41::builder()
                .with_host_capacity_support(
                    embedded_sdmmc::sdcard::argument::HostCapacitySupport::SdhcOrSdxc,
                )
                .with_fast_boot(false)
                .with_xpc(embedded_sdmmc::sdcard::argument::PowerControl::MaximumPerformance)
                .with_s18r(false)
                .with_ocr(VOLTAGE_LEVEL_CAPABILITIES)
                .build()
                .raw_value(),
        );
        loop {
            // Now poll until the card initialization is complete. In real driver code, timeout
            // handling or an upper polling limit might be a good idea.
            self.0.insert_acmd(AcmdId::ACMD41_SdSendOpCond, 0);
            let r3 = R3::new_with_raw_value(self.0.read_reply_u32());
            if r3.initialization_complete() {
                break;
            }
        }

        // Retrieve and cache the CID. This puts it into identification mode.
        self.0.insert_command(CmdId::CMD2_AllSendCid, 0);
        let cid_raw = self.0.read_reply_u128();
        let cid = embedded_sdmmc::sdcard::cid::Cid::new_with_raw_value(cid_raw);

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
                .with_bus_width(argument::BusWidth::_4bits)
                .build()
                .raw_value(),
        );

        Ok(SdCard {
            cid,
            csd,
            rca,
            sd_mock: self.0,
        })
    }
}

#[derive(Debug)]
pub struct SdCard {
    cid: Cid,
    csd: Csd,
    rca: u16,
    #[allow(unused)]
    sd_mock: SdCardMock,
}

const MOCK_SD_RCA: u16 = 1;

fn main() -> Result<(), anyhow::Error> {
    let sd_mock = SdCardMock::new(MOCK_SD_RCA);
    let sd_card_uninit = SdCardUninitialized::new(sd_mock);
    let sd_card = sd_card_uninit
        .initialize()
        .context("failed to initialize SD card")?;
    println!("SD card initialized successfully",);
    println!("--------");
    println!("Relative Card Address: {}", sd_card.rca);
    println!("--------");
    println!("CSD: {:?}", sd_card.csd);
    println!("--------");
    println!("CID: {:?}", sd_card.cid);
    Ok(())
}
