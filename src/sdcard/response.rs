//! # Card response module

use super::argument::VoltageSuppliedSelect;

/// SD card state.
#[bitbybit::bitenum(u4, exhaustive = false)]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
pub enum State {
    /// Idle state.
    Idle = 0,
    /// Ready state.
    Ready = 1,
    /// Identification state.
    Ident = 2,
    /// Standby state.
    Stby = 3,
    /// Transfer state.
    Tran = 4,
    /// Data state.
    Data = 5,
    /// Receive state.
    Rcv = 6,
    /// Programming state.
    Prg = 7,
    /// Disconnected state.
    Dis = 8,
    /// Reserved for IO mode.
    ReservedIoMode = 15,
}

/// Card status (R1).
#[bitbybit::bitfield(
    u32,
    default = 0x0,
    debug,
    defmt_fields(feature = "defmt-log"),
    forbid_overlaps
)]
pub struct CardStatus {
    /// The command's argument was out range of the allowed range for this card.
    #[bit(31, rw)]
    out_of_range: bool,
    /// A misaligned address which did not match the block length that was used in the command
    #[bit(30, rw)]
    address_error: bool,
    /// The transferred block length is not allowed for this card, or the number of transferred
    /// bytes does not match the block length.
    #[bit(29, rw)]
    block_len_error: bool,
    /// An error in the sequence of erase commands occurred.
    #[bit(28, rw)]
    erase_seq_error: bool,
    /// An invalid selection of write-blocks for erase occurred.
    #[bit(27, rw)]
    erase_param: bool,
    /// Set when the host attempts to write to a protected block or to the temporary write
    /// protected card or write protected until power cycle card or permanent write protected
    /// card.
    #[bit(26, rw)]
    wp_violation: bool,
    /// When set, signals that the card is locked by the host
    #[bit(25, rw)]
    card_is_locked: bool,
    /// Set when a sequence or password error has been detected in lock/unlock card command.
    #[bit(24, rw)]
    lock_unlock_failed: bool,
    /// The CRC check of the previous command failed.
    #[bit(23, rw)]
    com_crc_error: bool,
    /// Command not legal for the card state
    #[bit(22, rw)]
    illegal_command: bool,
    /// Card internal ECC was applied but failed to correct the data.
    #[bit(21, rw)]
    card_ecc_failed: bool,
    /// Internal card controller error
    #[bit(20, rw)]
    cc_error: bool,
    /// A general or an unknown error occurred during the operation.
    #[bit(19, rw)]
    error: bool,
    /// Can be either one of the following errors:
    /// - The read only section of the CSD does not match the card content.
    /// - An attempt to reverse the copy (set as original) or permanent WP (unprotected) bits was made.
    #[bit(16, rw)]
    csd_overwrite: bool,
    /// Set when only partial address space was erased due to existing write protected blocks
    /// or the temporary write protected or write protected until power cycle or permanent write
    /// protected card was erased.
    #[bit(15, rw)]
    wp_erase_skip: bool,
    /// The command has been executed without using the internal ECC.
    #[bit(14, rw)]
    card_ecc_disabled: bool,
    /// An erase sequence was cleared before executing because an out of erase sequence command
    /// was received
    #[bit(13, rw)]
    erase_reset: bool,
    /// CURRENT_STATE. The state of the card when receiving the command. If the command
    /// execution causes a state change, it will be visible to the host in the response to the
    /// next command.
    #[bits(9..=12, rw)]
    state: Option<State>,
    /// Corresponds to buffer empty signaling on the bus
    #[bit(8, rw)]
    ready_for_data: bool,
    /// FX_EVENT. Extension Functions may set this bit to get host to deal with events.
    #[bit(6, rw)]
    fx_event: bool,
    /// '1': Enabled. The card will expect ACMD, or an indiication that the command has been
    /// interpreted as ACMD.
    #[bit(5, rw)]
    app_cmd: bool,
    /// Error in the sequence of the authentification process.
    #[bit(3, rw)]
    ake_seq_error: bool,
}

/// Operation Conditions Register (OCR).
#[bitbybit::bitfield(
    u32,
    default = 0x0,
    debug,
    defmt_fields(feature = "defmt-log"),
    forbid_overlaps
)]
pub struct Ocr {
    /// '0' if not finished yet.
    #[bit(31, rw)]
    initialization_complete: bool,
    /// CCS. Only valid if power up status bit is set.
    #[bit(30, rw)]
    card_capacity_status: bool,
    /// UHS-II Card Status. '1' if card supports UHS-II interface.
    #[bit(29, rw)]
    uhs_2_card_status: bool,
    /// CO2T. Only supported by SDUC cards.
    #[bit(27, rw)]
    over_2_tb_support_status: bool,
    /// Switching to 1.8V accepted. Only supported by UHS-I cards.
    #[bit(24, rw)]
    s18a: bool,
    /// 3.5V-3.6V voltage window supported.
    #[bit(23, rw)]
    _3_5_to_3_6v: bool,
    /// 3.4V-3.5V voltage window supported.
    #[bit(22, rw)]
    _3_4_to_3_5v: bool,
    /// 3.3V-3.4V voltage window supported.
    #[bit(21, rw)]
    _3_3_to_3_4v: bool,
    /// 3.2V-3.3V voltage window supported.
    #[bit(20, rw)]
    _3_2_to_3_3v: bool,
    /// 3.1V-3.2V voltage window supported.
    #[bit(19, rw)]
    _3_1_to_3_2v: bool,
    /// 3.0V-3.1V voltage window supported.
    #[bit(18, rw)]
    _3_0_to_3_1v: bool,
    /// 2.9V-3.0V voltage window supported.
    #[bit(17, rw)]
    _2_9_to_3_0v: bool,
    /// 2.8V-2.9V voltage window supported.
    #[bit(16, rw)]
    _2_8_to_2_9v: bool,
    /// 2.7V-2.8V voltage window supported.
    #[bit(15, rw)]
    _2_7_to_2_8v: bool,
    /// Reserved for low voltage range.
    #[bit(7, rw)]
    reserved_low_voltage: bool,
}

/// R1 is the card sate.
pub type R1 = CardStatus;

/// R3 is the OCR register.
pub type R3 = Ocr;

/// Published RCA response (R6).
#[bitbybit::bitfield(
    u32,
    default = 0x0,
    debug,
    defmt_fields(feature = "defmt-log"),
    forbid_overlaps
)]
pub struct R6 {
    /// Relative card address.
    #[bits(16..=31, rw)]
    rca: u16,
    /// Status bit 23. The CRC check of the previous command failed.
    #[bit(15, rw)]
    com_crc_error: bool,
    /// Status bit 22. Command not legal for the card state
    #[bit(14, rw)]
    illegal_command: bool,
    /// Status bit 19. A general or an unknown error occurred during the operation.
    #[bit(13, rw)]
    error: bool,
    /// CURRENT_STATE. The state of the card when receiving the command. If the command
    /// execution causes a state change, it will be visible to the host in the response to the
    /// next command.
    #[bits(9..=12, rw)]
    state: Option<State>,
    /// Corresponds to buffer empty signaling on the bus
    #[bit(8, rw)]
    ready_for_data: bool,
    /// FX_EVENT. Extension Functions may set this bit to get host to deal with events.
    #[bit(6, rw)]
    fx_event: bool,
    /// '1': Enabled. The card will expect ACMD, or an indiication that the command has been
    /// interpreted as ACMD.
    #[bit(5, rw)]
    app_cmd: bool,
    /// Error in the sequence of the authentification process.
    #[bit(3, rw)]
    ake_seq_error: bool,
}

/// Card interface conditions (R7).
#[bitbybit::bitfield(
    u32,
    default = 0x0,
    debug,
    defmt_fields(feature = "defmt-log"),
    forbid_overlaps
)]
pub struct R7 {
    /// PCIe 1.2V support.
    #[bit(13, rw)]
    pcie_1_2v_support: bool,
    /// PCIe response.
    #[bit(12, rw)]
    pcie_accepted: bool,
    /// Voltage accepted.
    #[bits(8..=11, rw)]
    voltage_accepted: Option<VoltageSuppliedSelect>,
    /// Echo-back of check pattern.
    #[bits(0..=7, rw)]
    echo_check_pattern: u8,
}
