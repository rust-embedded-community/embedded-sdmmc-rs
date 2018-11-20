//! embedded-sdmmc-rs - SDMMC Protocol
//!
//! Implements the SD/MMC protocol on some generic SPI interface.

use super::sdmmc_proto::*;
use super::{Block, BlockDevice, BlockIdx};
use nb::block;

use super::Error;

const DEFAULT_DELAY_COUNT: u32 = 32;

pub struct SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    CS: embedded_hal::digital::OutputPin,
    <SPI as embedded_hal::spi::FullDuplex<u8>>::Error: core::fmt::Debug
{
    spi: SPI,
    cs: CS,
    card_type: CardType,
    state: State,
    delay_count: u32,
}

#[derive(Debug, Clone)]
pub enum SdMmcError {
    Transport,
    CantEnableCRC,
    Timeout,
    Cmd58Error,
    RegisterReadError,
    CrcError,
    ReadError,
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
    pub fn new(spi: SPI, cs: CS) -> SdMmcSpi<SPI, CS> {
        SdMmcSpi {
            spi,
            cs,
            card_type: CardType::SD1,
            state: State::NoInit,
            delay_count: DEFAULT_DELAY_COUNT
        }
    }

    pub fn spi(&mut self) -> &mut SPI {
        &mut self.spi
    }

    /// This routine must be performed with an SPI clock speed of around 100 - 400 kHz.
    /// Afterwards you may increase the SPI clock speed.
    pub fn init(&mut self) -> Result<(), Error<SdMmcSpi<SPI, CS>>> {
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
                return Err(Error::DeviceError(SdMmcError::CantEnableCRC));
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
                    return Err(Error::DeviceError(SdMmcError::Cmd58Error));
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

    pub fn card_size_bytes(&mut self) -> Result<u32, Error<SdMmcSpi<SPI, CS>>> {
        self.with_chip_select(|s| {
            if s.state != State::Idle {
                return Err(Error::DeviceError(SdMmcError::BadState));
            }
            let csd = s.read_csd()?;
            match csd {
                Csd::V1(ref contents) => Ok(contents.card_capacity_bytes()),
                Csd::V2(ref contents) => Ok(contents.card_capacity_bytes()),
            }
        })
    }

    fn with_chip_select<F, T>(&mut self, func: F) -> T where F: FnOnce(&mut Self) -> T {
        self.cs.set_low();
        let result = func(self);
        self.cs.set_low();
        result
    }

    fn delay_init(&mut self) {
        self.delay_count = DEFAULT_DELAY_COUNT;
    }

    fn delay(&mut self) -> Result<(), Error<SdMmcSpi<SPI, CS>>> {
        // Crude delay loop to avoid battering the card
        if self.delay_count == 0 {
            return Err(Error::DeviceError(SdMmcError::Timeout));
        } else {
            self.delay_count -= 1;
        }
        let foo: u32 = 0;
        for _ in 0..100_000 {
            unsafe { core::ptr::read_volatile(&foo) };
        }
        Ok(())
    }

    fn read_csd(&mut self) -> Result<Csd, Error<SdMmcSpi<SPI, CS>>> {
        match self.card_type {
            CardType::SD1 => {
                let mut csd = CsdV1::new();
                if self.card_command(CMD9, 0)? != 0 {
                    return Err(Error::DeviceError(SdMmcError::RegisterReadError));
                }
                self.read_data(&mut csd.data)?;
                Ok(Csd::V1(csd))
            }
            CardType::SD2 | CardType::SDHC => {
                let mut csd = CsdV2::new();
                if self.card_command(CMD9, 0)? != 0 {
                    return Err(Error::DeviceError(SdMmcError::RegisterReadError));
                }
                self.read_data(&mut csd.data)?;
                Ok(Csd::V2(csd))
            }
        }
    }

    fn read_data(&mut self, buffer: &mut [u8]) -> Result<(), Error<SdMmcSpi<SPI, CS>>> {
        let status = loop {
            let s = self.receive()?;
            if s != 0xFF {
                break s;
            }
            // TODO: Handle timeout here
        };
        if status != DATA_START_BLOCK {
            return Err(Error::DeviceError(SdMmcError::ReadError));
        }

        for b in buffer.iter_mut() {
            *b = self.receive()?;
        }

        let mut crc: u16 = self.receive()? as u16;
        crc <<= 8;
        crc |= self.receive()? as u16;

        if crc != crc16_ccitt(buffer) {
            return Err(Error::DeviceError(SdMmcError::CrcError));
        }

        Ok(())
    }

    fn card_acmd(&mut self, command: u8, arg: u32) -> Result<u8, Error<SdMmcSpi<SPI, CS>>> {
        self.card_command(CMD55, 0)?;
        self.card_command(command, arg)
    }

    fn card_command(&mut self, command: u8, arg: u32) -> Result<u8, Error<SdMmcSpi<SPI, CS>>> {
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

        Err(Error::DeviceError(SdMmcError::Timeout))
    }

    /// Receive a byte from the SD card by clocking in an 0xFF byte.
    fn receive(&mut self) -> Result<u8, Error<SdMmcSpi<SPI, CS>>> {
        self.transfer(0xFF)
    }

    /// Send a byte from the SD card.
    fn send(&mut self, out: u8) -> Result<(), Error<SdMmcSpi<SPI, CS>>> {
        let _ = self.transfer(out)?;
        Ok(())
    }

    /// Send one byte and receive one byte.
    fn transfer(&mut self, out: u8) -> Result<u8, Error<SdMmcSpi<SPI, CS>>> {
        block!(self.spi.send(out)).map_err(|_e| Error::DeviceError(SdMmcError::Transport))?;
        block!(self.spi.read()).map_err(|_e| Error::DeviceError(SdMmcError::Transport))
    }

    fn wait_not_busy(&mut self) -> Result<(), Error<SdMmcSpi<SPI, CS>>> {
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

    pub fn card_size(&mut self) -> Result<BlockIdx, Error<SdMmcSpi<SPI, CS>>> {
        unimplemented!()
    }
    pub fn erase(
        &mut self,
        _first_block: BlockIdx,
        _last_block: BlockIdx,
    ) -> Result<(), Error<SdMmcSpi<SPI, CS>>> {
        unimplemented!()
    }
}

impl<SPI, CS> BlockDevice for SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    <SPI as embedded_hal::spi::FullDuplex<u8>>::Error: core::fmt::Debug,
    CS: embedded_hal::digital::OutputPin,
{
    type Error = SdMmcError;

    /// Read one or more blocks, starting at the given block index.
    fn read(
        &mut self,
        _blocks: &mut [Block],
        _start_block_idx: BlockIdx,
    ) -> Result<(), Self::Error> {
        unimplemented!();
    }
    /// Write one or more blocks, starting at the given block index.
    fn write(&mut self, _blocks: &[Block], _start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        unimplemented!();
    }
    /// Complete a multi-block transaction and return the SD card to idle mode.
    fn sync(&mut self) -> Result<(), Self::Error> {
        unimplemented!();
    }
}
