//! Implements the BlockDevice trait for an SD/MMC Protocol over SPI.
//!
//! This is currently optimised for readability and debugability, not
//! performance.

use super::{proto::*, AcquireOpts, CardType, Delay, Error};
use crate::blockdevice::{Block, BlockCount, BlockIdx};
use crate::{debug, trace, warn};

/// Inner details for the SD Card driver.
///
/// All the APIs required `&mut self`.
pub struct SpiSdCardInner<SPI, DELAYER>
where
    SPI: embedded_hal::spi::SpiDevice<u8>,
    DELAYER: embedded_hal::delay::DelayNs,
{
    spi: SPI,
    delayer: DELAYER,
    card_type: Option<CardType>,
    options: AcquireOpts,
}

impl<SPI, DELAYER> SpiSdCardInner<SPI, DELAYER>
where
    SPI: embedded_hal::spi::SpiDevice<u8>,
    DELAYER: embedded_hal::delay::DelayNs,
{
    /// Construct a new raw SPI transport interface for SD/MMC Card.
    pub fn new(spi: SPI, delayer: DELAYER, options: AcquireOpts) -> Self {
        SpiSdCardInner {
            spi,
            delayer,
            card_type: None,
            options,
        }
    }
    /// Get a temporary borrow on the underlying SPI device.
    pub fn spi<T, F>(&mut self, func: F) -> T
    where
        F: FnOnce(&mut SPI) -> T,
    {
        func(&mut self.spi)
    }
    /// Read one or more blocks, starting at the given block index.
    pub fn read(&mut self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Error> {
        let start_idx = match self.card_type {
            Some(CardType::SD1 | CardType::SD2) => start_block_idx.0 * 512,
            Some(CardType::SDHC) => start_block_idx.0,
            None => return Err(Error::CardNotFound),
        };

        if blocks.len() == 1 {
            // Start a single-block read
            self.card_command(CMD17, start_idx)?;
            self.read_data(&mut blocks[0].contents)?;
        } else {
            // Start a multi-block read
            self.card_command(CMD18, start_idx)?;
            for block in blocks.iter_mut() {
                self.read_data(&mut block.contents)?;
            }
            // Stop the read
            self.card_command(CMD12, 0)?;
        }
        Ok(())
    }

    /// Write one or more blocks, starting at the given block index.
    pub fn write(&mut self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Error> {
        let start_idx = match self.card_type {
            Some(CardType::SD1 | CardType::SD2) => start_block_idx.0 * 512,
            Some(CardType::SDHC) => start_block_idx.0,
            None => return Err(Error::CardNotFound),
        };
        if blocks.len() == 1 {
            // Start a single-block write
            self.card_command(CMD24, start_idx)?;
            self.write_data(DATA_START_BLOCK, &blocks[0].contents)?;
            self.wait_not_busy(Delay::new_write())?;
            if self.card_command(CMD13, 0)? != 0x00 {
                return Err(Error::WriteError);
            }
            if self.read_byte()? != 0x00 {
                return Err(Error::WriteError);
            }
        } else {
            // > It is recommended using this command preceding CMD25, some of the cards will be faster for Multiple
            // > Write Blocks operation. Note that the host should send ACMD23 just before WRITE command if the host
            // > wants to use the pre-erased feature
            self.card_acmd(ACMD23, blocks.len() as u32)?;
            // wait for card to be ready before sending the next command
            self.wait_not_busy(Delay::new_write())?;

            // Start a multi-block write
            self.card_command(CMD25, start_idx)?;
            for block in blocks.iter() {
                self.wait_not_busy(Delay::new_write())?;
                self.write_data(WRITE_MULTIPLE_TOKEN, &block.contents)?;
            }
            // Stop the write
            self.wait_not_busy(Delay::new_write())?;
            self.write_byte(STOP_TRAN_TOKEN)?;
        }
        Ok(())
    }

    /// Determine how many blocks this device can hold.
    pub fn num_blocks(&mut self) -> Result<BlockCount, Error> {
        let csd = self.read_csd()?;
        debug!("CSD: {:?}", csd);
        let num_blocks = match csd {
            Csd::V1(ref contents) => contents.card_capacity_blocks(),
            Csd::V2(ref contents) => contents.card_capacity_blocks(),
        };
        Ok(BlockCount(num_blocks))
    }

    /// Return the usable size of this SD card in bytes.
    pub fn num_bytes(&mut self) -> Result<u64, Error> {
        let csd = self.read_csd()?;
        debug!("CSD: {:?}", csd);
        match csd {
            Csd::V1(ref contents) => Ok(contents.card_capacity_bytes()),
            Csd::V2(ref contents) => Ok(contents.card_capacity_bytes()),
        }
    }

    /// Can this card erase single blocks?
    pub fn erase_single_block_enabled(&mut self) -> Result<bool, Error> {
        let csd = self.read_csd()?;
        match csd {
            Csd::V1(ref contents) => Ok(contents.erase_single_block_enabled()),
            Csd::V2(ref contents) => Ok(contents.erase_single_block_enabled()),
        }
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

    /// Read an arbitrary number of bytes from the card using the SD Card
    /// protocol and an optional CRC. Always fills the given buffer, so make
    /// sure it's the right size.
    fn read_data(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        // Get first non-FF byte.
        let mut delay = Delay::new_read();
        let status = loop {
            let s = self.read_byte()?;
            if s != 0xFF {
                break s;
            }
            delay.delay(&mut self.delayer, Error::TimeoutReadBuffer)?;
        };
        if status != DATA_START_BLOCK {
            return Err(Error::ReadError);
        }

        buffer.fill(0xFF);
        self.transfer_bytes(buffer)?;

        // These two bytes are always sent. They are either a valid CRC, or
        // junk, depending on whether CRC mode was enabled.
        let mut crc_bytes = [0xFF; 2];
        self.transfer_bytes(&mut crc_bytes)?;
        if self.options.use_crc {
            let crc = u16::from_be_bytes(crc_bytes);
            let calc_crc = crc16(buffer);
            if crc != calc_crc {
                return Err(Error::CrcError(crc, calc_crc));
            }
        }

        Ok(())
    }

    /// Write an arbitrary number of bytes to the card using the SD protocol and
    /// an optional CRC.
    fn write_data(&mut self, token: u8, buffer: &[u8]) -> Result<(), Error> {
        self.write_byte(token)?;
        self.write_bytes(buffer)?;
        let crc_bytes = if self.options.use_crc {
            crc16(buffer).to_be_bytes()
        } else {
            [0xFF, 0xFF]
        };
        // These two bytes are always sent. They are either a valid CRC, or
        // junk, depending on whether CRC mode was enabled.
        self.write_bytes(&crc_bytes)?;

        let status = self.read_byte()?;
        if (status & DATA_RES_MASK) != DATA_RES_ACCEPTED {
            Err(Error::WriteError)
        } else {
            Ok(())
        }
    }

    /// Check the card is initialised.
    pub fn check_init(&mut self) -> Result<(), Error> {
        if self.card_type.is_none() {
            // If we don't know what the card type is, try and initialise the
            // card. This will tell us what type of card it is.
            self.acquire()
        } else {
            Ok(())
        }
    }

    /// Initializes the card into a known state (or at least tries to).
    pub fn acquire(&mut self) -> Result<(), Error> {
        debug!("acquiring card with opts: {:?}", self.options);
        let f = |s: &mut Self| {
            // Assume it hasn't worked
            let mut card_type;
            trace!("Reset card..");
            // Enter SPI mode.
            let mut delay = Delay::new(s.options.acquire_retries);
            for _attempts in 1.. {
                trace!("Enter SPI mode, attempt: {}..", _attempts);
                match s.card_command(CMD0, 0) {
                    Err(Error::TimeoutCommand(0)) => {
                        // Try again?
                        warn!("Timed out, trying again..");
                        // Try flushing the card as done here: https://github.com/greiman/SdFat/blob/master/src/SdCard/SdSpiCard.cpp#L170,
                        // https://github.com/rust-embedded-community/embedded-sdmmc-rs/pull/65#issuecomment-1270709448
                        for _ in 0..0xFF {
                            s.write_byte(0xFF)?;
                        }
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
                    }
                }

                delay.delay(&mut s.delayer, Error::CardNotFound)?;
            }
            // Enable CRC
            debug!("Enable CRC: {}", s.options.use_crc);
            // "The SPI interface is initialized in the CRC OFF mode in default"
            // -- SD Part 1 Physical Layer Specification v9.00, Section 7.2.2 Bus Transfer Protection
            if s.options.use_crc && s.card_command(CMD59, 1)? != R1_IDLE_STATE {
                return Err(Error::CantEnableCRC);
            }
            // Check card version
            let mut delay = Delay::new_command();
            let arg = loop {
                if s.card_command(CMD8, 0x1AA)? == (R1_ILLEGAL_COMMAND | R1_IDLE_STATE) {
                    card_type = CardType::SD1;
                    break 0;
                }
                let mut buffer = [0xFF; 4];
                s.transfer_bytes(&mut buffer)?;
                let status = buffer[3];
                if status == 0xAA {
                    card_type = CardType::SD2;
                    break 0x4000_0000;
                }
                delay.delay(&mut s.delayer, Error::TimeoutCommand(CMD8))?;
            };

            let mut delay = Delay::new_command();
            while s.card_acmd(ACMD41, arg)? != R1_READY_STATE {
                delay.delay(&mut s.delayer, Error::TimeoutACommand(ACMD41))?;
            }

            if card_type == CardType::SD2 {
                if s.card_command(CMD58, 0)? != 0 {
                    return Err(Error::Cmd58Error);
                }
                let mut buffer = [0xFF; 4];
                s.transfer_bytes(&mut buffer)?;
                if (buffer[0] & 0xC0) == 0xC0 {
                    card_type = CardType::SDHC;
                }
                // Ignore the other three bytes
            }
            debug!("Card version: {:?}", card_type);
            s.card_type = Some(card_type);
            Ok(())
        };
        let result = f(self);
        let _ = self.read_byte();
        result
    }

    /// Mark the card as requiring a reset.
    ///
    /// The next operation will assume the card has been freshly inserted.
    pub fn mark_card_uninit(&mut self) {
        self.card_type = None;
    }

    /// Get the card type.
    pub fn get_card_type(&self) -> Option<CardType> {
        self.card_type
    }

    /// Tell the driver the card has been initialised.
    pub unsafe fn mark_card_as_init(&mut self, card_type: CardType) {
        self.card_type = Some(card_type);
    }

    /// Perform an application-specific command.
    fn card_acmd(&mut self, command: u8, arg: u32) -> Result<u8, Error> {
        self.card_command(CMD55, 0)?;
        self.card_command(command, arg)
    }

    /// Perform a command.
    fn card_command(&mut self, command: u8, arg: u32) -> Result<u8, Error> {
        if command != CMD0 && command != CMD12 {
            self.wait_not_busy(Delay::new_command())?;
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

        self.write_bytes(&buf)?;

        // skip stuff byte for stop read
        if command == CMD12 {
            let _result = self.read_byte()?;
        }

        let mut delay = Delay::new_command();
        loop {
            let result = self.read_byte()?;
            if (result & 0x80) == ERROR_OK {
                return Ok(result);
            }
            delay.delay(&mut self.delayer, Error::TimeoutCommand(command))?;
        }
    }

    /// Receive a byte from the SPI bus by clocking out an 0xFF byte.
    fn read_byte(&mut self) -> Result<u8, Error> {
        self.transfer_byte(0xFF)
    }

    /// Send a byte over the SPI bus and ignore what comes back.
    fn write_byte(&mut self, out: u8) -> Result<(), Error> {
        let _ = self.transfer_byte(out)?;
        Ok(())
    }

    /// Send one byte and receive one byte over the SPI bus.
    fn transfer_byte(&mut self, out: u8) -> Result<u8, Error> {
        let mut read_buf = [0u8; 1];
        self.spi
            .transfer(&mut read_buf, &[out])
            .map_err(|_| Error::Transport)?;
        Ok(read_buf[0])
    }

    /// Send multiple bytes and ignore what comes back over the SPI bus.
    fn write_bytes(&mut self, out: &[u8]) -> Result<(), Error> {
        self.spi.write(out).map_err(|_e| Error::Transport)?;
        Ok(())
    }

    /// Send multiple bytes and replace them with what comes back over the SPI bus.
    fn transfer_bytes(&mut self, in_out: &mut [u8]) -> Result<(), Error> {
        self.spi
            .transfer_in_place(in_out)
            .map_err(|_e| Error::Transport)?;
        Ok(())
    }

    /// Spin until the card returns 0xFF, or we spin too many times and
    /// timeout.
    fn wait_not_busy(&mut self, mut delay: Delay) -> Result<(), Error> {
        loop {
            let s = self.read_byte()?;
            if s == 0xFF {
                break;
            }
            delay.delay(&mut self.delayer, Error::TimeoutWaitNotBusy)?;
        }
        Ok(())
    }
}
