//! # Argument module
//!
//! Arguments are additional command parameters. You can use the `sd_card_init` example to see
//! how these data structures can be used during a SD card initialization process.

use arbitrary_int::u24;

/// Voltage settings supplied in CMD8.
#[bitbybit::bitenum(u4, exhaustive = false)]
#[derive(Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
pub enum VoltageSuppliedSelect {
    /// Regular voltage range.
    #[default]
    _2_7To3_6V = 0b0001,
    /// Reserved for low voltage.
    LowVoltageRange = 0b0010,
}

/// CMD8 argument.
#[bitbybit::bitfield(u32, default = 0x0, debug, defmt_fields(feature = "defmt-log"))]
pub struct Cmd8 {
    /// PCIe v1.2 support.
    #[bit(13, rw)]
    pcie_1_2v_support: bool,
    /// PCIe available.
    #[bit(12, rw)]
    pcie_availability: bool,
    /// Voltage settings.
    #[bits(8..=11, rw)]
    voltage_supplied: Option<VoltageSuppliedSelect>,
    /// Check pattern which is echoed.
    #[bits(0..=7, rw)]
    check_pattern: u8,
}

/// Power control (XPC) settings.
#[bitbybit::bitenum(u1, exhaustive = true)]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
pub enum PowerControl {
    /// Power saving.
    PowerSaving = 0,
    /// Maximum performance.
    MaximumPerformance = 1,
}

/// HPC.
#[bitbybit::bitenum(u1, exhaustive = true)]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
pub enum HostCapacitySupport {
    /// SDSC only.
    SdscOnly = 0,
    /// Extended capacity - SDHC or SDXC.
    SdhcOrSdxc = 1,
}

/// Lower OCR bits used to negotiate voltage capabilities.
#[bitbybit::bitfield(u24, default = 0x0, debug, defmt_fields(feature = "defmt-log"))]
pub struct OcrLower {
    /// 3.5 to 3.6V
    #[bit(23, rw)]
    _3_5_to_3_6v: bool,
    /// 3.4 to 3.5V
    #[bit(22, rw)]
    _3_4_to_3_5v: bool,
    /// 3.3 to 3.4V
    #[bit(21, rw)]
    _3_3_to_3_4v: bool,
    /// 3.2 to 3.3V
    #[bit(20, rw)]
    _3_2_to_3_3v: bool,
    /// 3.1 to 3.2V
    #[bit(19, rw)]
    _3_1_to_3_2v: bool,
    /// 3.0 to 3.1V
    #[bit(18, rw)]
    _3_0_to_3_1v: bool,
    /// 2.9 to 3.0V
    #[bit(17, rw)]
    _2_9_to_3_0v: bool,
    /// 2.8 to 2.9V
    #[bit(16, rw)]
    _2_8_to_2_9v: bool,
    /// 2.7 to 2.8V
    #[bit(15, rw)]
    _2_7_to_2_8v: bool,
    /// Reserved for low voltage
    #[bit(7, rw)]
    reserved_low_voltage: bool,
}

/// Bus width setting.
#[bitbybit::bitenum(u2, exhaustive = false)]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
pub enum BusWidth {
    /// 1 bit bus.
    _1bit = 0b00,
    /// 4 bit bus.
    _4bits = 0b10,
}

/// ACMD6 - Set bus width.
#[bitbybit::bitfield(
    u32,
    default = 0x0,
    debug,
    defmt_fields(feature = "defmt-log"),
    forbid_overlaps
)]
pub struct Acmd6 {
    /// Bus width.
    #[bits(0..=1, rw)]
    bus_width: Option<BusWidth>,
}

/// ACMD41 argument - Capability negotiation.
#[bitbybit::bitfield(
    u32,
    default = 0x0,
    debug,
    defmt_fields(feature = "defmt-log"),
    forbid_overlaps
)]
pub struct Acmd41 {
    /// HPC.
    #[bit(30, rw)]
    host_capacity_support: HostCapacitySupport,
    /// FB.
    #[bit(29, rw)]
    fast_boot: bool,
    /// XPC.
    #[bit(28, rw)]
    xpc: PowerControl,
    /// Switch to 1.8V signal voltage request.
    #[bit(24, rw)]
    s18r: bool,
    /// If this is set to 0. ACMD41 is called "inquire CMD41" that does not start
    /// initialization and is used for getting OCR. Bits 24 to 31 are ignored.
    #[bits(0..=23, rw)]
    ocr: OcrLower,
}

/// Relative Card Address (RCA) select.
#[bitbybit::bitfield(
    u32,
    default = 0x0,
    debug,
    defmt_bitfields(feature = "defmt-log"),
    forbid_overlaps
)]
pub struct RcaSelect {
    /// RCA.
    #[bits(16..=31, rw)]
    rca: u16,
}

/// CMD9 argument.
pub type Cmd9 = RcaSelect;
/// CMD7 argument.
pub type Cmd7 = RcaSelect;

/// CMD13 argument.
#[bitbybit::bitfield(
    u32,
    default = 0x0,
    debug,
    defmt_bitfields(feature = "defmt-log"),
    forbid_overlaps
)]
pub struct Cmd13 {
    /// RCA.
    #[bits(16..=31, rw)]
    rca: u16,
    /// Send task status instead of regular card status.
    #[bit(15, rw)]
    send_task_status: bool,
}
