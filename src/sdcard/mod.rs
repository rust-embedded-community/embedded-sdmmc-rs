//! The SD/MMC Protocol
//!
//! Implements the SD/MMC protocol on some generic SPI interface.
//!
//! This is currently optimised for readability and debugability, not
//! performance.

pub mod proto;

use crate::{trace, Block, BlockCount, BlockDevice, BlockIdx};
use core::cell::RefCell;
use proto::*;

// =============================================================================
// Imports
// =============================================================================

use crate::{debug, warn};

// =============================================================================
// Constants
// =============================================================================

const DEFAULT_DELAY_COUNT: u32 = 32_000;

// =============================================================================
// Types and Implementations
// =============================================================================

/// Represents an SD Card on an SPI bus.
///
/// Built from an SPI peripheral and a Chip Select pin. We need Chip Select to
/// be separate so we can clock out some bytes without Chip Select asserted
/// (which puts the card into SPI mode).
///
/// All the APIs take `&self` - mutability is handled using an inner `RefCell`.
pub struct SdCard<SPI, CS, DELAYER>
where
    SPI: embedded_hal::blocking::spi::Transfer<u8>,
    CS: embedded_hal::digital::v2::OutputPin,
    <SPI as embedded_hal::blocking::spi::Transfer<u8>>::Error: core::fmt::Debug,
    DELAYER: embedded_hal::blocking::delay::DelayUs<u8>,
{
    inner: RefCell<SdCardInner<SPI, CS, DELAYER>>,
}

impl<SPI, CS, DELAYER> SdCard<SPI, CS, DELAYER>
where
    SPI: embedded_hal::blocking::spi::Transfer<u8>,
    CS: embedded_hal::digital::v2::OutputPin,
    <SPI as embedded_hal::blocking::spi::Transfer<u8>>::Error: core::fmt::Debug,
    DELAYER: embedded_hal::blocking::delay::DelayUs<u8>,
{
    /// Create a new SD/MMC Card driver using a raw SPI interface.
    ///
    /// Uses the default options.
    pub fn new(spi: SPI, cs: CS, delayer: DELAYER) -> SdCard<SPI, CS, DELAYER> {
        Self::new_with_options(spi, cs, delayer, AcquireOpts::default())
    }

    /// Construct a new SD/MMC Card driver, using a raw SPI interface and the given options.
    pub fn new_with_options(
        spi: SPI,
        cs: CS,
        delayer: DELAYER,
        options: AcquireOpts,
    ) -> SdCard<SPI, CS, DELAYER> {
        SdCard {
            inner: RefCell::new(SdCardInner {
                spi,
                cs,
                delayer,
                card_type: None,
                options,
            }),
        }
    }

    /// Get a temporary borrow on the underlying SPI device. Useful if you
    /// need to re-clock the SPI.
    pub fn spi<T, F>(&self, func: F) -> T
    where
        F: FnOnce(&mut SPI) -> T,
    {
        let mut inner = self.inner.borrow_mut();
        func(&mut inner.spi)
    }

    /// Return the usable size of this SD card in bytes.
    pub fn num_bytes(&self) -> Result<u64, Error> {
        let mut inner = self.inner.borrow_mut();
        inner.check_init()?;
        inner.num_bytes()
    }

    /// Can this card erase single blocks?
    pub fn erase_single_block_enabled(&self) -> Result<bool, Error> {
        let mut inner = self.inner.borrow_mut();
        inner.check_init()?;
        inner.erase_single_block_enabled()
    }

    /// Mark the card as requiring a reset.
    ///
    /// The next operation will assume the card has been freshly inserted.
    pub fn mark_card_uninit(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.card_type = None;
    }

    /// Get the card type.
    pub fn get_card_type(&self) -> Option<CardType> {
        let inner = self.inner.borrow();
        inner.card_type
    }

    /// Tell the driver the card has been initialised.
    ///
    /// # Safety
    ///
    /// Only do this if the card has actually been initialised and is of the
    /// indicated type, otherwise corruption may occur.
    pub unsafe fn mark_card_as_init(&self, card_type: CardType) {
        let mut inner = self.inner.borrow_mut();
        inner.card_type = Some(card_type);
    }
}

impl<SPI, CS, DELAYER> BlockDevice for SdCard<SPI, CS, DELAYER>
where
    SPI: embedded_hal::blocking::spi::Transfer<u8>,
    <SPI as embedded_hal::blocking::spi::Transfer<u8>>::Error: core::fmt::Debug,
    CS: embedded_hal::digital::v2::OutputPin,
    DELAYER: embedded_hal::blocking::delay::DelayUs<u8>,
{
    type Error = Error;

    /// Read one or more blocks, starting at the given block index.
    fn read(
        &self,
        blocks: &mut [Block],
        start_block_idx: BlockIdx,
        _reason: &str,
    ) -> Result<(), Self::Error> {
        let mut inner = self.inner.borrow_mut();
        debug!(
            "Read {} blocks @ {} for {}",
            blocks.len(),
            start_block_idx.0,
            _reason
        );
        inner.check_init()?;
        inner.read(blocks, start_block_idx)
    }

    /// Write one or more blocks, starting at the given block index.
    fn write(&self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        let mut inner = self.inner.borrow_mut();
        debug!("Writing {} blocks @ {}", blocks.len(), start_block_idx.0);
        inner.check_init()?;
        inner.write(blocks, start_block_idx)
    }

    /// Determine how many blocks this device can hold.
    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        let mut inner = self.inner.borrow_mut();
        inner.check_init()?;
        inner.num_blocks()
    }
}

/// Represents an SD Card on an SPI bus.
///
/// All the APIs required `&mut self`.
struct SdCardInner<SPI, CS, DELAYER>
where
    SPI: embedded_hal::blocking::spi::Transfer<u8>,
    CS: embedded_hal::digital::v2::OutputPin,
    <SPI as embedded_hal::blocking::spi::Transfer<u8>>::Error: core::fmt::Debug,
    DELAYER: embedded_hal::blocking::delay::DelayUs<u8>,
{
    spi: SPI,
    cs: CS,
    delayer: DELAYER,
    card_type: Option<CardType>,
    options: AcquireOpts,
}

impl<SPI, CS, DELAYER> SdCardInner<SPI, CS, DELAYER>
where
    SPI: embedded_hal::blocking::spi::Transfer<u8>,
    CS: embedded_hal::digital::v2::OutputPin,
    <SPI as embedded_hal::blocking::spi::Transfer<u8>>::Error: core::fmt::Debug,
    DELAYER: embedded_hal::blocking::delay::DelayUs<u8>,
{
    /// Read one or more blocks, starting at the given block index.
    fn read(&mut self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Error> {
        let start_idx = match self.card_type {
            Some(CardType::SD1 | CardType::SD2) => start_block_idx.0 * 512,
            Some(CardType::SDHC) => start_block_idx.0,
            None => return Err(Error::CardNotFound),
        };
        self.with_chip_select(|s| {
            if blocks.len() == 1 {
                // Start a single-block read
                s.card_command(CMD17, start_idx)?;
                s.read_data(&mut blocks[0].contents)?;
            } else {
                // Start a multi-block read
                s.card_command(CMD18, start_idx)?;
                for block in blocks.iter_mut() {
                    s.read_data(&mut block.contents)?;
                }
                // Stop the read
                s.card_command(CMD12, 0)?;
            }
            Ok(())
        })
    }

    /// Write one or more blocks, starting at the given block index.
    fn write(&mut self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Error> {
        let start_idx = match self.card_type {
            Some(CardType::SD1 | CardType::SD2) => start_block_idx.0 * 512,
            Some(CardType::SDHC) => start_block_idx.0,
            None => return Err(Error::CardNotFound),
        };
        self.with_chip_select(|s| {
            if blocks.len() == 1 {
                // Start a single-block write
                s.card_command(CMD24, start_idx)?;
                s.write_data(DATA_START_BLOCK, &blocks[0].contents)?;
                s.wait_not_busy()?;
                if s.card_command(CMD13, 0)? != 0x00 {
                    return Err(Error::WriteError);
                }
                if s.receive()? != 0x00 {
                    return Err(Error::WriteError);
                }
            } else {
                // Start a multi-block write
                s.card_command(CMD25, start_idx)?;
                for block in blocks.iter() {
                    s.wait_not_busy()?;
                    s.write_data(WRITE_MULTIPLE_TOKEN, &block.contents)?;
                }
                // Stop the write
                s.wait_not_busy()?;
                s.send(STOP_TRAN_TOKEN)?;
            }
            Ok(())
        })
    }

    /// Determine how many blocks this device can hold.
    fn num_blocks(&mut self) -> Result<BlockCount, Error> {
        let num_blocks = self.with_chip_select(|s| {
            let csd = s.read_csd()?;
            debug!("CSD: {:?}", csd);
            match csd {
                Csd::V1(ref contents) => Ok(contents.card_capacity_blocks()),
                Csd::V2(ref contents) => Ok(contents.card_capacity_blocks()),
            }
        })?;
        Ok(BlockCount(num_blocks))
    }

    /// Return the usable size of this SD card in bytes.
    fn num_bytes(&mut self) -> Result<u64, Error> {
        self.with_chip_select(|s| {
            let csd = s.read_csd()?;
            debug!("CSD: {:?}", csd);
            match csd {
                Csd::V1(ref contents) => Ok(contents.card_capacity_bytes()),
                Csd::V2(ref contents) => Ok(contents.card_capacity_bytes()),
            }
        })
    }

    /// Can this card erase single blocks?
    pub fn erase_single_block_enabled(&mut self) -> Result<bool, Error> {
        self.with_chip_select(|s| {
            let csd = s.read_csd()?;
            match csd {
                Csd::V1(ref contents) => Ok(contents.erase_single_block_enabled()),
                Csd::V2(ref contents) => Ok(contents.erase_single_block_enabled()),
            }
        })
    }

    /// Read the 'card specific data' block.
    fn read_csd(&mut self) -> Result<Csd, Error> {
        match self.card_type {
            Some(CardType::SD1) => {
                let mut csd = CsdV1::new();
                if self.card_command(CMD9, 0)? != 0 {
                    return Err(Error::RegisterReadError);
                }
                self.read_data(&mut csd.data)?;
                Ok(Csd::V1(csd))
            }
            Some(CardType::SD2 | CardType::SDHC) => {
                let mut csd = CsdV2::new();
                if self.card_command(CMD9, 0)? != 0 {
                    return Err(Error::RegisterReadError);
                }
                self.read_data(&mut csd.data)?;
                Ok(Csd::V2(csd))
            }
            None => Err(Error::CardNotFound),
        }
    }

    /// Read an arbitrary number of bytes from the card. Always fills the
    /// given buffer, so make sure it's the right size.
    fn read_data(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        // Get first non-FF byte.
        let mut delay = Delay::new();
        let status = loop {
            let s = self.receive()?;
            if s != 0xFF {
                break s;
            }
            delay.delay(&mut self.delayer, Error::TimeoutReadBuffer)?;
        };
        if status != DATA_START_BLOCK {
            return Err(Error::ReadError);
        }

        for b in buffer.iter_mut() {
            *b = 0xFF;
        }
        self.spi.transfer(buffer).map_err(|_e| Error::Transport)?;

        let mut crc = u16::from(self.receive()?);
        crc <<= 8;
        crc |= u16::from(self.receive()?);

        let calc_crc = crc16(buffer);
        if crc != calc_crc {
            return Err(Error::CrcError(crc, calc_crc));
        }

        Ok(())
    }

    /// Write an arbitrary number of bytes to the card.
    fn write_data(&mut self, token: u8, buffer: &[u8]) -> Result<(), Error> {
        let calc_crc = crc16(buffer);
        self.send(token)?;
        for &b in buffer.iter() {
            self.send(b)?;
        }
        self.send((calc_crc >> 8) as u8)?;
        self.send(calc_crc as u8)?;
        let status = self.receive()?;
        if (status & DATA_RES_MASK) != DATA_RES_ACCEPTED {
            Err(Error::WriteError)
        } else {
            Ok(())
        }
    }

    fn cs_high(&mut self) -> Result<(), Error> {
        self.cs.set_high().map_err(|_| Error::GpioError)
    }

    fn cs_low(&mut self) -> Result<(), Error> {
        self.cs.set_low().map_err(|_| Error::GpioError)
    }

    /// Check the card is initialised.
    fn check_init(&mut self) -> Result<(), Error> {
        if self.card_type.is_none() {
            // If we don't know what the card type is, try and initialise the
            // card. This will tell us what type of card it is.
            self.acquire()
        } else {
            Ok(())
        }
    }

    /// Initializes the card into a known state (or at least tries to).
    fn acquire(&mut self) -> Result<(), Error> {
        debug!("acquiring card with opts: {:?}", self.options);
        let f = |s: &mut Self| {
            // Assume it hasn't worked
            let mut card_type;
            trace!("Reset card..");
            // Supply minimum of 74 clock cycles without CS asserted.
            s.cs_high()?;
            for _ in 0..10 {
                s.send(0xFF)?;
            }
            // Assert CS
            s.cs_low()?;
            // Enter SPI mode
            let mut delay = Delay::new();
            let mut attempts = 32;
            while attempts > 0 {
                trace!("Enter SPI mode, attempt: {}..", 32i32 - attempts);

                match s.card_command(CMD0, 0) {
                    Err(Error::TimeoutCommand(0)) => {
                        // Try again?
                        warn!("Timed out, trying again..");
                        // Try flushing the card as done here: https://github.com/greiman/SdFat/blob/master/src/SdCard/SdSpiCard.cpp#L170,
                        // https://github.com/rust-embedded-community/embedded-sdmmc-rs/pull/65#issuecomment-1270709448
                        for _ in 0..0xFF {
                            s.send(0xFF)?;
                        }
                        attempts -= 1;
                    }
                    Err(e) => {
                        return Err(e);
                    }
                    Ok(R1_IDLE_STATE) => {
                        break;
                    }
                    Ok(_r) => {
                        // Try again
                        warn!("Got response: {:x}, trying again..", _r);
                        attempts -= 1;
                    }
                }

                delay.delay(&mut s.delayer, Error::TimeoutCommand(CMD0))?;
            }
            if attempts == 0 {
                return Err(Error::CardNotFound);
            }
            // Enable CRC
            debug!("Enable CRC: {}", s.options.require_crc);
            if s.card_command(CMD59, 1)? != R1_IDLE_STATE && s.options.require_crc {
                return Err(Error::CantEnableCRC);
            }
            // Check card version
            let mut delay = Delay::new();
            let arg = loop {
                if s.card_command(CMD8, 0x1AA)? == (R1_ILLEGAL_COMMAND | R1_IDLE_STATE) {
                    card_type = CardType::SD1;
                    break 0;
                }
                s.receive()?;
                s.receive()?;
                s.receive()?;
                let status = s.receive()?;
                if status == 0xAA {
                    card_type = CardType::SD2;
                    break 0x4000_0000;
                }
                delay.delay(&mut s.delayer, Error::TimeoutCommand(CMD8))?;
            };

            let mut delay = Delay::new();
            while s.card_acmd(ACMD41, arg)? != R1_READY_STATE {
                delay.delay(&mut s.delayer, Error::TimeoutACommand(ACMD41))?;
            }

            if card_type == CardType::SD2 {
                if s.card_command(CMD58, 0)? != 0 {
                    return Err(Error::Cmd58Error);
                }
                if (s.receive()? & 0xC0) == 0xC0 {
                    card_type = CardType::SDHC;
                }
                // Discard other three bytes
                s.receive()?;
                s.receive()?;
                s.receive()?;
            }
            debug!("Card version: {:?}", card_type);
            s.card_type = Some(card_type);
            Ok(())
        };
        let result = f(self);
        self.cs_high()?;
        let _ = self.receive();
        result
    }

    /// Perform a function that might error with the chipselect low.
    /// Always releases the chipselect, even if the function errors.
    fn with_chip_select<F, T>(&mut self, func: F) -> Result<T, Error>
    where
        F: FnOnce(&mut Self) -> Result<T, Error>,
    {
        self.cs_low()?;
        let result = func(self);
        self.cs_high()?;
        result
    }

    /// Perform an application-specific command.
    fn card_acmd(&mut self, command: u8, arg: u32) -> Result<u8, Error> {
        self.card_command(CMD55, 0)?;
        self.card_command(command, arg)
    }

    /// Perform a command.
    fn card_command(&mut self, command: u8, arg: u32) -> Result<u8, Error> {
        if command != CMD0 && command != CMD12 {
            self.wait_not_busy()?;
        }

        let mut buf = [
            0x40 | command,
            (arg >> 24) as u8,
            (arg >> 16) as u8,
            (arg >> 8) as u8,
            arg as u8,
            0,
        ];
        buf[5] = crc7(&buf[0..5]);

        for b in buf.iter() {
            self.send(*b)?;
        }

        // skip stuff byte for stop read
        if command == CMD12 {
            let _result = self.receive()?;
        }

        for _ in 0..512 {
            let result = self.receive()?;
            if (result & 0x80) == ERROR_OK {
                return Ok(result);
            }
        }

        Err(Error::TimeoutCommand(command))
    }

    /// Receive a byte from the SD card by clocking in an 0xFF byte.
    fn receive(&mut self) -> Result<u8, Error> {
        self.transfer(0xFF)
    }

    /// Send a byte from the SD card.
    fn send(&mut self, out: u8) -> Result<(), Error> {
        let _ = self.transfer(out)?;
        Ok(())
    }

    /// Send one byte and receive one byte.
    fn transfer(&mut self, out: u8) -> Result<u8, Error> {
        self.spi
            .transfer(&mut [out])
            .map(|b| b[0])
            .map_err(|_e| Error::Transport)
    }

    /// Spin until the card returns 0xFF, or we spin too many times and
    /// timeout.
    fn wait_not_busy(&mut self) -> Result<(), Error> {
        let mut delay = Delay::new();
        loop {
            let s = self.receive()?;
            if s == 0xFF {
                break;
            }
            delay.delay(&mut self.delayer, Error::TimeoutWaitNotBusy)?;
        }
        Ok(())
    }
}

/// Options for acquiring the card.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug)]
pub struct AcquireOpts {
    /// Some cards don't support CRC mode. At least a 512MiB Transcend one.
    pub require_crc: bool,
}

impl Default for AcquireOpts {
    fn default() -> Self {
        AcquireOpts { require_crc: true }
    }
}

/// The possible errors this crate can generate.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Copy, Clone)]
pub enum Error {
    /// We got an error from the SPI peripheral
    Transport,
    /// We failed to enable CRC checking on the SD card
    CantEnableCRC,
    /// We didn't get a response when reading data from the card
    TimeoutReadBuffer,
    /// We didn't get a response when waiting for the card to not be busy
    TimeoutWaitNotBusy,
    /// We didn't get a response when executing this command
    TimeoutCommand(u8),
    /// We didn't get a response when executing this application-specific command
    TimeoutACommand(u8),
    /// We got a bad response from Command 58
    Cmd58Error,
    /// We failed to read the Card Specific Data register
    RegisterReadError,
    /// We got a CRC mismatch (card gave us, we calculated)
    CrcError(u16, u16),
    /// Error reading from the card
    ReadError,
    /// Error writing to the card
    WriteError,
    /// Can't perform this operation with the card in this state
    BadState,
    /// Couldn't find the card
    CardNotFound,
    /// Couldn't set a GPIO pin
    GpioError,
}

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
    /// An high-capacity 'SDHC' Card.
    ///  
    /// Uses block-addressing internally to support capacities above 2GiB.
    SDHC,
}

/// A terrible hack for busy-waiting the CPU while we wait for the card to
/// sort itself out.
///
/// @TODO replace this!
struct Delay {
    count: u32,
}

impl Delay {
    fn new() -> Delay {
        Delay {
            count: DEFAULT_DELAY_COUNT,
        }
    }

    fn delay<T>(&mut self, delayer: &mut T, err: Error) -> Result<(), Error>
    where
        T: embedded_hal::blocking::delay::DelayUs<u8>,
    {
        if self.count == 0 {
            Err(err)
        } else {
            delayer.delay_us(10);
            self.count -= 1;
            Ok(())
        }
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
