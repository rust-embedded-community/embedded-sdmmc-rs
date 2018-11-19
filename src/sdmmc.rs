//! embedded-sdmmc-rs - SDMMC Protocol
//!
//! Implements the SD/MMC protocol on some generic SPI interface.

use super::sdmmc_proto::*;
use super::{Block, BlockDevice, BlockIdx};
use nb::block;

use super::Error;

pub struct SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    CS: embedded_hal::digital::OutputPin,
    <SPI as embedded_hal::spi::FullDuplex<u8>>::Error: core::fmt::Debug
{
    spi: SPI,
    cs: CS,
    card_type: CardType,
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
        }
    }

    pub fn spi(&mut self) -> &mut SPI {
        &mut self.spi
    }

    /// This routine must be performed with an SPI clock speed of around 100 - 400 kHz.
    /// Afterwards you may increase the SPI clock speed.
    pub fn init(&mut self) -> Result<(), Error<SdMmcSpi<SPI, CS>>> {
        let result = self.inner_init();
        self.cs.set_high();
        let _ = self.send(0xFF);
        result
    }

    fn inner_init(&mut self) -> Result<(), Error<SdMmcSpi<SPI, CS>>> {
        // Supply minimum of 74 clock cycles without CS asserted.
        self.cs.set_high();
        for _ in 0..10 {
            self.send(0xFF)?;
        }
        // Assert CS
        self.cs.set_low();
        // Enter SPI mode
        while self.card_command(CMD0, 0)? != R1_IDLE_STATE {
            // TODO: Check for timeout
            // Crude delay loop to avoid battering the card
            let foo: u32 = 0;
            for _ in 0..2_000_000 {
                unsafe { core::ptr::read_volatile(&foo) };
            }
        }
        // Enable CRC
        if self.card_command(CMD59, 1)? != R1_IDLE_STATE {
            return Err(Error::DeviceError(SdMmcError::CantEnableCRC));
        }
        // Check card version
        loop {
            if self.card_command(CMD8, 0x1AA)? == (R1_ILLEGAL_COMMAND | R1_IDLE_STATE) {
                self.card_type = CardType::SD1;
                break;
            }
            self.receive()?;
            self.receive()?;
            self.receive()?;
            let status = self.receive()?;
            if status == 0xAA {
                self.card_type = CardType::SD2;
                break;
            }
            // TODO: Check for timeout
        }

        let arg = match self.card_type {
            CardType::SD1 => 0,
            CardType::SD2 | CardType::SDHC => 0x40000000,
        };

        while self.card_acmd(ACMD41, arg)? != R1_READY_STATE {
            // TODO: Check for timeout
        }

        if self.card_type == CardType::SD2 {
            if self.card_command(CMD58, 0)? != 0 {
                return Err(Error::DeviceError(SdMmcError::Cmd58Error));
            }
            if (self.receive()? & 0xC0) == 0xC0 {
                self.card_type = CardType::SDHC;
            }
            // Discard other three bytes
            self.receive()?;
            self.receive()?;
            self.receive()?;
        }
        Ok(())
    }

    pub fn card_size_bytes(&mut self) -> Result<u32, Error<SdMmcSpi<SPI, CS>>> {
        let csd = self.read_csd()?;
        match csd {
            Csd::V1(ref contents) => Ok(contents.card_capacity_bytes()),
            Csd::V2(ref contents) => Ok(contents.card_capacity_bytes()),
        }
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

    pub fn card_command(&mut self, command: u8, arg: u32) -> Result<u8, Error<SdMmcSpi<SPI, CS>>> {
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
        loop {
            let s = self.receive()?;
            if s == 0xFF {
                break;
            }
            // TODO: Handle timeout here
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
