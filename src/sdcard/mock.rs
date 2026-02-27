//! SD card mock module.
//!
//! The `sd_card_init` example application shows how the mock is used to initialize a SD card.
//! This can serve as a reference for implementing this for real devices.

use crate::sdcard::{
    AcmdId,
    argument::{Acmd6, BusWidth, Cmd7, Cmd8, Cmd9, Cmd13},
    response::{R1, R3, R6, R7},
};

/// CID retreived from a real SD card, will be returned by the mock object.
pub const TEST_CID: [u8; 16] = [
    0x12, 0x34, 0x56, 0x41, 0x53, 0x54, 0x43, 0x0, 0x20, 0x0, 0x0, 0xc, 0xef, 0x1, 0x65, 0xef,
];
/// CSD retreived from a real SD card, will be returned by the mock object.
pub const TEST_CSD: [u8; 16] = [
    0x40, 0x0E, 0x00, 0x32, 0x5B, 0x59, 0x00, 0x00, 0x1D, 0x69, 0x7F, 0x80, 0x0A, 0x40, 0x00, 0x8B,
];

/// The IDLE mode has some private substates used for operating condition negotiation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum IdleSubState {
    /// Idle substate.
    Idle,
    /// Received CMD8, ready to accept ACMD41.
    ReceivedIfCond,
    /// Received ACMD41 with OCR bit, initializing itself now.
    ///
    /// This allows for the intialization to take multiple polling calls.
    InitializingSelf {
        /// Step counter. When a treshold is reached, initialization is complete.
        step: u8,
    },
}
/// SD card mock states.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum State {
    /// Idle state.
    Idle(IdleSubState),
    /// Ready state.
    Ready,
    /// Identification state.
    Ident,
    /// Standby state.
    Stby,
    /// Transfer state.
    Tran,
    /// Data state.
    Data,
    /// Receive state.
    Rcv,
    /// Programming state.
    Prg,
    /// Disconnected state.
    Dis,
}

impl From<State> for super::response::State {
    fn from(value: State) -> Self {
        use super::response;
        match value {
            State::Idle(_) => response::State::Idle,
            State::Ready => response::State::Ready,
            State::Ident => response::State::Ident,
            State::Stby => response::State::Stby,
            State::Tran => response::State::Tran,
            State::Data => response::State::Data,
            State::Rcv => response::State::Rcv,
            State::Prg => response::State::Prg,
            State::Dis => response::State::Dis,
        }
    }
}

/// SD card mock.
///
/// This basically behaves like a virtual SD card. Currently, this mock simulates a Ver2.00 or later
/// SDHC memory card. As such, it responds to the CMD8 command as well. Furthermore, while this
/// mock is capable of performing a transition until the [super::response::State::Tran] transmission
/// state is reached, it does not implement / mock file operations yet.
#[derive(Debug, PartialEq, Eq)]
pub struct SdCardMock {
    state: State,
    //mode: Mode,
    rca: u16,
    csd: [u8; 16],
    cid: [u8; 16],
    bus_width: BusWidth,
    polling_calls_card_init: u8,
    reply_buf: [u8; 16],
}

impl SdCardMock {
    /// Create a new SD card mock in the idle state.
    pub fn new(relative_card_addr: u16) -> Self {
        Self {
            state: State::Idle(IdleSubState::Idle),
            rca: relative_card_addr,
            csd: [0; 16],
            cid: [0; 16],
            bus_width: BusWidth::_1bit,
            polling_calls_card_init: 3,
            reply_buf: [0; 16],
        }
    }

    /// Insert a [super::AcmdId] command into the SD card.
    ///
    /// Normall, this would involve sending a [super::CmdId::CMD55_AppCmd] first, but this
    /// function simplifies the process and allows inserting the ACMD directly.
    /// In a real implementation, it might be useful to implement a similar function.
    pub fn insert_acmd(&mut self, acmd_id: super::AcmdId, argument: u32) {
        match acmd_id {
            AcmdId::ACMD41_SdSendOpCond => {
                self.handle_amcd41(argument);
            }
            AcmdId::ACMD6_SetBusWidth => {
                let argument = Acmd6::new_with_raw_value(argument);
                if let Ok(bus_width) = argument.bus_width() {
                    self.bus_width = bus_width;
                }
            }
            _ => {}
        }
    }

    /// Insert a [super::CmdId] command into the SD card.
    ///
    /// In a real implementation, it might be useful to implement a similar function which uses
    /// the controller hardware to trasnsfer [super::CmdId] to the SD card.
    pub fn insert_command(&mut self, cmd_id: super::CmdId, argument: u32) {
        match cmd_id {
            super::CmdId::CMD0_GoIdleState => self.state = State::Idle(IdleSubState::Idle),
            super::CmdId::CMD2_AllSendCid => {
                if self.state == State::Ready {
                    self.reply_buf.copy_from_slice(&TEST_CID);
                    self.state = State::Ident
                }
            }
            super::CmdId::CMD3_SendRelativeAddr => {
                if self.state == State::Ident {
                    let response = R6::builder()
                        .with_rca(self.rca)
                        .with_com_crc_error(false)
                        .with_illegal_command(false)
                        .with_error(false)
                        .with_state(super::response::State::Ident)
                        .with_ready_for_data(false)
                        .with_fx_event(false)
                        .with_app_cmd(false)
                        .with_ake_seq_error(false)
                        .build();
                    self.write_u32_reply(response.raw_value());
                    self.state = State::Stby;
                }
            }
            super::CmdId::CMD7_SelectCard => {
                let argument = Cmd7::new_with_raw_value(argument);
                if argument.rca() == self.rca {
                    let response = R1::ZERO.with_state(super::response::State::Stby);
                    self.write_u32_reply(response.raw_value());
                    self.state = State::Tran;
                }
            }
            super::CmdId::CMD13_SendStatus => {
                let argument = Cmd13::new_with_raw_value(argument);
                if argument.rca() == self.rca {
                    let response = R1::ZERO.with_state(self.state.into());
                    self.write_u32_reply(response.raw_value());
                    self.state = State::Tran;
                }
            }
            super::CmdId::CMD9_SendCsd => {
                let argument = Cmd9::new_with_raw_value(argument);
                if argument.rca() == self.rca {
                    self.reply_buf.copy_from_slice(&TEST_CSD);
                }
            }
            super::CmdId::CMD8_SendIfCond => {
                let argument = Cmd8::new_with_raw_value(argument);
                if matches!(self.state, State::Idle { .. }) {
                    self.state = State::Idle(IdleSubState::ReceivedIfCond);
                    let reply = R7::builder()
                        .with_pcie_1_2v_support(false)
                        .with_pcie_accepted(false)
                        .with_echo_check_pattern(argument.check_pattern())
                        .with_voltage_accepted(super::argument::VoltageSuppliedSelect::_2_7To3_6V)
                        .build();
                    self.write_u32_reply(reply.raw_value());
                }
            }

            _ => (),
        }
    }

    /// Read [u32] reply from the SD card mock.
    pub fn read_reply_u32(&self) -> u32 {
        u32::from_be_bytes(self.reply_buf[0..4].try_into().unwrap())
    }

    /// Read [u128] reply from the SD card mock.
    pub fn read_reply_u128(&self) -> u128 {
        u128::from_be_bytes(self.reply_buf)
    }

    fn handle_amcd41(&mut self, argument: u32) {
        let argument = super::argument::Acmd41::new_with_raw_value(argument);
        // If any OCR bits are set,
        let ocr_bits_set = argument.ocr().raw_value().value() != 0;
        if let State::Idle(substate) = self.state {
            match substate {
                // We ignore this if we have not received CMD8 before.
                IdleSubState::Idle => (),
                IdleSubState::ReceivedIfCond => {
                    if ocr_bits_set {
                        let response = R3::ZERO;
                        self.write_u32_reply(response.raw_value());
                        self.state = State::Idle(IdleSubState::InitializingSelf { step: 1 });
                    }
                }
                IdleSubState::InitializingSelf { step } => {
                    if ocr_bits_set {
                        // I have no idea what a SD casrd does for this. It might start
                        // the initialization again. We just expect users to use the polling
                        // mode properly.
                        let response = R3::ZERO;
                        self.write_u32_reply(response.raw_value());
                        self.state = State::Idle(IdleSubState::InitializingSelf { step: 1 });
                    } else {
                        if step == self.polling_calls_card_init {
                            let response = R3::builder()
                                .with__2_7_to_2_8v(false)
                                .with__2_8_to_2_9v(false)
                                .with__2_9_to_3_0v(false)
                                .with__3_0_to_3_1v(false)
                                .with__3_1_to_3_2v(false)
                                .with__3_2_to_3_3v(true)
                                .with__3_3_to_3_4v(true)
                                .with__3_4_to_3_5v(false)
                                .with__3_5_to_3_6v(false)
                                .with_reserved_low_voltage(false)
                                .with_uhs_2_card_status(false)
                                // Initialization is completed immediately for the mock. We could configure
                                // the mock to simulate the initailization taking multiple polling calls.
                                .with_initialization_complete(true)
                                .with_card_capacity_status(true)
                                .with_over_2_tb_support_status(false)
                                .with_s18a(argument.s18r())
                                .build();
                            self.write_u32_reply(response.raw_value());
                            self.state = State::Ready;
                        } else {
                            self.state =
                                State::Idle(IdleSubState::InitializingSelf { step: step + 1 });
                        }
                    }
                }
            }
        }
    }

    fn write_u32_reply(&mut self, value: u32) {
        self.reply_buf[0..4].copy_from_slice(&value.to_be_bytes());
    }
}
