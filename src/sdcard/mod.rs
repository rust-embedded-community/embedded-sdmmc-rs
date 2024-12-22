//! Implements the BlockDevice trait for an SD/MMC Protocol.
//!
//! This is currently optimised for readability and debugability, not
//! performance.

pub mod proto;
mod spi;
pub use spi::SpiTransport;

use crate::{Block, BlockCount, BlockDevice, BlockIdx};
use core::cell::RefCell;

// ****************************************************************************
// Imports
// ****************************************************************************

use crate::debug;

// ****************************************************************************
// Types and Implementations
// ****************************************************************************

/// Driver for an SD Card on an SPI bus.
///
/// Built from an [`SpiDevice`] implementation and a Chip Select pin.
///
/// Before talking to the SD Card, the caller needs to send 74 clocks cycles on
/// the SPI Clock line, at 400 kHz, with no chip-select asserted (or at least,
/// not the chip-select of the SD Card).
///
/// This kind of breaks the embedded-hal model, so how to do this is left to
/// the caller. You could drive the SpiBus directly, or use an SpiDevice with
/// a dummy chip-select pin. Or you could try just not doing the 74 clocks and
/// see if your card works anyway - some do, some don't.
///
/// All the APIs take `&self` - mutability is handled using an inner `RefCell`.
///
/// [`SpiDevice`]: embedded_hal::spi::SpiDevice
pub struct SdCard<T: Transport> {
    inner: RefCell<T>,
}

impl<SPI, DELAYER> SdCard<SpiTransport<SPI, DELAYER>>
where
    SPI: embedded_hal::spi::SpiDevice<u8>,
    DELAYER: embedded_hal::delay::DelayNs,
{
    /// Create a new SD/MMC Card driver using a raw SPI interface.
    ///
    /// The card will not be initialised at this time. Initialisation is
    /// deferred until a method is called on the object.
    ///
    /// Uses the default options.
    pub fn new_spi(spi: SPI, delayer: DELAYER) -> Self {
        Self::new_spi_with_options(spi, delayer, AcquireOpts::default())
    }

    /// Construct a new SD/MMC Card driver, using a raw SPI interface and the given options.
    ///
    /// See the docs of the [`SdCard`] struct for more information about
    /// how to construct the needed `SPI` and `CS` types.
    ///
    /// The card will not be initialised at this time. Initialisation is
    /// deferred until a method is called on the object.
    pub fn new_spi_with_options(spi: SPI, delayer: DELAYER, options: AcquireOpts) -> Self {
        SdCard {
            inner: RefCell::new(spi::SpiTransport::new(spi, delayer, options)),
        }
    }

    /// Get a temporary borrow on the underlying SPI device.
    ///
    /// The given closure will be called exactly once, and will be passed a
    /// mutable reference to the underlying SPI object.
    ///
    /// Useful if you need to re-clock the SPI, but does not perform card
    /// initialisation.
    pub fn spi<T, F>(&self, func: F) -> T
    where
        F: FnOnce(&mut SPI) -> T,
    {
        let mut inner = self.inner.borrow_mut();
        inner.spi(func)
    }
}

impl<T: Transport> SdCard<T> {
    /// Return the usable size of this SD card in bytes.
    ///
    /// This will trigger card (re-)initialisation.
    pub fn num_bytes(&self) -> Result<u64, Error> {
        let mut inner = self.inner.borrow_mut();
        inner.check_init()?;
        inner.num_bytes()
    }

    /// Can this card erase single blocks?
    ///
    /// This will trigger card (re-)initialisation.
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
        inner.mark_card_uninit();
    }

    /// Get the card type.
    ///
    /// This will trigger card (re-)initialisation.
    pub fn get_card_type(&self) -> Option<CardType> {
        let mut inner = self.inner.borrow_mut();
        inner.check_init().ok()?;
        inner.get_card_type()
    }

    /// Tell the driver the card has been initialised.
    ///
    /// This is here in case you were previously using the SD Card, and then a
    /// previous instance of this object got destroyed but you know for certain
    /// the SD Card remained powered up and initialised, and you'd just like to
    /// read/write to/from the card again without going through the
    /// initialisation sequence again.
    ///
    /// # Safety
    ///
    /// Only do this if the SD Card has actually been initialised. That is, if
    /// you have been through the card initialisation sequence as specified in
    /// the SD Card Specification by sending each appropriate command in turn,
    /// either manually or using another variable of this [`SdCard`]. The card
    /// must also be of the indicated type. Failure to uphold this will cause
    /// data corruption.
    pub unsafe fn mark_card_as_init(&self, card_type: CardType) {
        let mut inner = self.inner.borrow_mut();
        inner.mark_card_as_init(card_type);
    }
}

impl<T: Transport> BlockDevice for SdCard<T> {
    type Error = Error;

    /// Read one or more blocks, starting at the given block index.
    ///
    /// This will trigger card (re-)initialisation.
    fn read(&self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        let mut inner = self.inner.borrow_mut();
        debug!("Read {} blocks @ {}", blocks.len(), start_block_idx.0,);
        inner.check_init()?;
        inner.read(blocks, start_block_idx)
    }

    /// Write one or more blocks, starting at the given block index.
    ///
    /// This will trigger card (re-)initialisation.
    fn write(&self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        let mut inner = self.inner.borrow_mut();
        debug!("Writing {} blocks @ {}", blocks.len(), start_block_idx.0);
        inner.check_init()?;
        inner.write(blocks, start_block_idx)
    }

    /// Determine how many blocks this device can hold.
    ///
    /// This will trigger card (re-)initialisation.
    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        let mut inner = self.inner.borrow_mut();
        inner.check_init()?;
        inner.num_blocks()
    }
}

/// Abstract SD card transportation interface.
pub trait Transport {
    /// Read one or more blocks, starting at the given block index.
    fn read(&mut self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Error>;
    /// Write one or more blocks, starting at the given block index.
    fn write(&mut self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Error>;
    /// Determine how many blocks this device can hold.
    fn num_blocks(&mut self) -> Result<BlockCount, Error>;
    /// Return the usable size of this SD card in bytes.
    fn num_bytes(&mut self) -> Result<u64, Error>;
    /// Can this card erase single blocks?
    fn erase_single_block_enabled(&mut self) -> Result<bool, Error>;
    /// Check the card is initialised.
    fn check_init(&mut self) -> Result<(), Error>;
    /// Mark the card as requiring a reset.
    ///
    /// The next operation will assume the card has been freshly inserted.
    fn mark_card_uninit(&mut self);
    /// Get the card type.
    fn get_card_type(&self) -> Option<CardType>;
    /// Tell the driver the card has been initialised.
    unsafe fn mark_card_as_init(&mut self, card_type: CardType);
}

/// Options for acquiring the card.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug)]
pub struct AcquireOpts {
    /// Set to true to enable CRC checking on reading/writing blocks of data.
    ///
    /// Set to false to disable the CRC. Some cards don't support CRC correctly
    /// and this option may be useful in that instance.
    ///
    /// On by default because without it you might get silent data corruption on
    /// your card.
    pub use_crc: bool,

    /// Sets the number of times we will retry to acquire the card before giving up and returning
    /// `Err(Error::CardNotFound)`. By default, card acquisition will be retried 50 times.
    pub acquire_retries: u32,
}

impl Default for AcquireOpts {
    fn default() -> Self {
        AcquireOpts {
            use_crc: true,
            acquire_retries: 50,
        }
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

/// This an object you can use to busy-wait with a timeout.
///
/// Will let you call `delay` up to `max_retries` times before `delay` returns
/// an error.
struct Delay {
    retries_left: u32,
}

impl Delay {
    /// The default number of retries for a read operation.
    ///
    /// At ~10us each this is ~100ms.
    ///
    /// See `Part1_Physical_Layer_Simplified_Specification_Ver9.00-1.pdf` Section 4.6.2.1
    pub const DEFAULT_READ_RETRIES: u32 = 10_000;

    /// The default number of retries for a write operation.
    ///
    /// At ~10us each this is ~500ms.
    ///
    /// See `Part1_Physical_Layer_Simplified_Specification_Ver9.00-1.pdf` Section 4.6.2.2
    pub const DEFAULT_WRITE_RETRIES: u32 = 50_000;

    /// The default number of retries for a control command.
    ///
    /// At ~10us each this is ~100ms.
    ///
    /// No value is given in the specification, so we pick the same as the read timeout.
    pub const DEFAULT_COMMAND_RETRIES: u32 = 10_000;

    /// Create a new Delay object with the given maximum number of retries.
    fn new(max_retries: u32) -> Delay {
        Delay {
            retries_left: max_retries,
        }
    }

    /// Create a new Delay object with the maximum number of retries for a read operation.
    fn new_read() -> Delay {
        Delay::new(Self::DEFAULT_READ_RETRIES)
    }

    /// Create a new Delay object with the maximum number of retries for a write operation.
    fn new_write() -> Delay {
        Delay::new(Self::DEFAULT_WRITE_RETRIES)
    }

    /// Create a new Delay object with the maximum number of retries for a command operation.
    fn new_command() -> Delay {
        Delay::new(Self::DEFAULT_COMMAND_RETRIES)
    }

    /// Wait for a while.
    ///
    /// Checks the retry counter first, and if we hit the max retry limit, the
    /// value `err` is returned. Otherwise we wait for 10us and then return
    /// `Ok(())`.
    fn delay<T>(&mut self, delayer: &mut T, err: Error) -> Result<(), Error>
    where
        T: embedded_hal::delay::DelayNs,
    {
        if self.retries_left == 0 {
            Err(err)
        } else {
            delayer.delay_us(10);
            self.retries_left -= 1;
            Ok(())
        }
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
