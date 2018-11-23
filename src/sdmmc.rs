//! embedded-sdmmc-rs - SDMMC Protocol
//!
//! Implements the SD/MMC protocol on some generic SPI interface.
//!
//! This is currently optimised for readability and debugability, not
//! performance.

use super::sdmmc_proto::*;
use super::{Block, BlockDevice, BlockIdx};
use nb::block;

const DEFAULT_DELAY_COUNT: u32 = 32;

pub struct SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    CS: embedded_hal::digital::OutputPin,
    <SPI as embedded_hal::spi::FullDuplex<u8>>::Error: core::fmt::Debug,
{
    spi: SPI,
    cs: CS,
    card_type: CardType,
    state: State,
    delay_count: u32,
}

#[derive(Debug, Copy, Clone)]
pub enum Error {
    Transport,
    CantEnableCRC,
    Timeout,
    Cmd58Error,
    RegisterReadError,
    CrcError(u16, u16),
    ReadError,
    WriteError,
    BadState,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum State {
    NoInit,
    Error,
    Idle,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum CardType {
    SD1,
    SD2,
    SDHC,
}

impl<SPI, CS> SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    CS: embedded_hal::digital::OutputPin,
    <SPI as embedded_hal::spi::FullDuplex<u8>>::Error: core::fmt::Debug,
{
    /// Create a new SD/MMC controller using a raw SPI interface.
    pub fn new(spi: SPI, cs: CS) -> SdMmcSpi<SPI, CS> {
        SdMmcSpi {
            spi,
            cs,
            card_type: CardType::SD1,
            state: State::NoInit,
            delay_count: DEFAULT_DELAY_COUNT,
        }
    }

    /// Get a temporary borrow on the underlying SPI device. Useful if you
    /// need to re-clock the SPI after performing `init()`.
    pub fn spi(&mut self) -> &mut SPI {
        &mut self.spi
    }

    /// This routine must be performed with an SPI clock speed of around 100 - 400 kHz.
    /// Afterwards you may increase the SPI clock speed.
    pub fn init(&mut self) -> Result<(), Error> {
        let f = |s: &mut Self| {
            // Assume it hasn't worked
            s.state = State::Error;
            // Supply minimum of 74 clock cycles without CS asserted.
            s.cs.set_high();
            for _ in 0..10 {
                s.send(0xFF)?;
            }
            // Assert CS
            s.cs.set_low();
            // Enter SPI mode
            s.delay_init();
            while s.card_command(CMD0, 0)? != R1_IDLE_STATE {
                s.delay()?;
            }
            // Enable CRC
            if s.card_command(CMD59, 1)? != R1_IDLE_STATE {
                return Err(Error::CantEnableCRC);
            }
            // Check card version
            s.delay_init();
            loop {
                if s.card_command(CMD8, 0x1AA)? == (R1_ILLEGAL_COMMAND | R1_IDLE_STATE) {
                    s.card_type = CardType::SD1;
                    break;
                }
                s.receive()?;
                s.receive()?;
                s.receive()?;
                let status = s.receive()?;
                if status == 0xAA {
                    s.card_type = CardType::SD2;
                    break;
                }
                s.delay()?;
            }

            let arg = match s.card_type {
                CardType::SD1 => 0,
                CardType::SD2 | CardType::SDHC => 0x40000000,
            };

            s.delay_init();
            while s.card_acmd(ACMD41, arg)? != R1_READY_STATE {
                s.delay()?;
            }

            if s.card_type == CardType::SD2 {
                if s.card_command(CMD58, 0)? != 0 {
                    return Err(Error::Cmd58Error);
                }
                if (s.receive()? & 0xC0) == 0xC0 {
                    s.card_type = CardType::SDHC;
                }
                // Discard other three bytes
                s.receive()?;
                s.receive()?;
                s.receive()?;
            }
            s.state = State::Idle;
            Ok(())
        };
        let result = f(self);
        self.cs.set_high();
        let _ = self.receive();
        result
    }

    /// Return the usable size of this SD card in bytes.
    pub fn card_size_bytes(&mut self) -> Result<u64, Error> {
        self.check_state()?;
        self.with_chip_select(|s| {
            let csd = s.read_csd()?;
            match csd {
                Csd::V1(ref contents) => Ok(contents.card_capacity_bytes()),
                Csd::V2(ref contents) => Ok(contents.card_capacity_bytes()),
            }
        })
    }

    /// Erase some blocks on the card.
    pub fn erase(&mut self, _first_block: BlockIdx, _last_block: BlockIdx) -> Result<(), Error> {
        self.check_state()?;
        unimplemented!();
    }

    /// Can this card erase single blocks?
    pub fn erase_single_block_enabled(&mut self) -> Result<bool, Error> {
        self.check_state()?;
        self.with_chip_select(|s| {
            let csd = s.read_csd()?;
            match csd {
                Csd::V1(ref contents) => Ok(contents.erase_single_block_enabled()),
                Csd::V2(ref contents) => Ok(contents.erase_single_block_enabled()),
            }
        })
    }

    /// Return an error if we're not in  `State::Idle`. It probably means
    /// they haven't called `begin()`.
    fn check_state(&mut self) -> Result<(), Error> {
        if self.state != State::Idle {
            Err(Error::BadState)
        } else {
            Ok(())
        }
    }

    /// Perform a function that might error with the chipselect low.
    /// Always releases the chipselect, even if the function errors.
    fn with_chip_select<F, T>(&mut self, func: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.cs.set_low();
        let result = func(self);
        self.cs.set_low();
        result
    }

    /// Call this at the start of a delay loop.
    fn delay_init(&mut self) {
        self.delay_count = DEFAULT_DELAY_COUNT;
    }

    /// Call this in the delay loop.
    fn delay(&mut self) -> Result<(), Error> {
        // Crude delay loop to avoid battering the card
        if self.delay_count == 0 {
            return Err(Error::Timeout);
        } else {
            self.delay_count -= 1;
        }
        let foo: u32 = 0;
        for _ in 0..100_000 {
            unsafe { core::ptr::read_volatile(&foo) };
        }
        Ok(())
    }

    /// Read the 'card specific data' block.
    fn read_csd(&mut self) -> Result<Csd, Error> {
        match self.card_type {
            CardType::SD1 => {
                let mut csd = CsdV1::new();
                if self.card_command(CMD9, 0)? != 0 {
                    return Err(Error::RegisterReadError);
                }
                self.read_data(&mut csd.data)?;
                Ok(Csd::V1(csd))
            }
            CardType::SD2 | CardType::SDHC => {
                let mut csd = CsdV2::new();
                if self.card_command(CMD9, 0)? != 0 {
                    return Err(Error::RegisterReadError);
                }
                self.read_data(&mut csd.data)?;
                Ok(Csd::V2(csd))
            }
        }
    }

    /// Read an arbitrary number of bytes from the card. Always fills the
    /// given buffer, so make sure it's the right size.
    fn read_data(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        // Get first non-FF byte.
        self.delay_init();
        let status = loop {
            let s = self.receive()?;
            if s != 0xFF {
                break s;
            }
            self.delay()?;
        };
        if status != DATA_START_BLOCK {
            return Err(Error::ReadError);
        }

        for b in buffer.iter_mut() {
            *b = self.receive()?;
        }

        let mut crc: u16 = self.receive()? as u16;
        crc <<= 8;
        crc |= self.receive()? as u16;

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
        self.send((calc_crc >> 16) as u8)?;
        self.send((calc_crc >> 0) as u8)?;
        let status = self.receive()?;
        if (status & DATA_RES_MASK) != DATA_RES_ACCEPTED {
            Err(Error::WriteError)
        } else {
            Ok(())
        }
    }

    /// Perform an application-specific command.
    fn card_acmd(&mut self, command: u8, arg: u32) -> Result<u8, Error> {
        self.card_command(CMD55, 0)?;
        self.card_command(command, arg)
    }

    /// Perform a command.
    fn card_command(&mut self, command: u8, arg: u32) -> Result<u8, Error> {
        self.wait_not_busy()?;
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

        for _ in 0..255 {
            let result = self.receive()?;
            if (result & 0x80) == 0 {
                return Ok(result);
            }
        }

        Err(Error::Timeout)
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
        block!(self.spi.send(out)).map_err(|_e| Error::Transport)?;
        block!(self.spi.read()).map_err(|_e| Error::Transport)
    }

    /// Spin until the card returns 0xFF, or we spin too many times and
    /// timeout.
    fn wait_not_busy(&mut self) -> Result<(), Error> {
        self.delay_init();
        loop {
            let s = self.receive()?;
            if s == 0xFF {
                break;
            }
            self.delay()?;
        }
        Ok(())
    }
}

impl<SPI, CS> BlockDevice for SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    <SPI as embedded_hal::spi::FullDuplex<u8>>::Error: core::fmt::Debug,
    CS: embedded_hal::digital::OutputPin,
{
    type Error = Error;

    /// Read one or more blocks, starting at the given block index.
    fn read(&mut self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        let start_idx = match self.card_type {
            CardType::SD1 | CardType::SD2 => start_block_idx.0 * 512,
            CardType::SDHC => start_block_idx.0,
        };
        self.check_state()?;
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
    fn write(&mut self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        let start_idx = match self.card_type {
            CardType::SD1 | CardType::SD2 => start_block_idx.0 * 512,
            CardType::SDHC => start_block_idx.0,
        };
        self.check_state()?;
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
}
