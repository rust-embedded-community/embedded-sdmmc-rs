//! embedded-sdmmc-rs - SDMMC Protocol
//!
//! Implements the SD/MMC protocol on some generic SPI interface.

use super::{Block, BlockDevice, BlockIdx};
use super::sdmmc_proto::*;
use nb::block;

use super::Error;

pub struct SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    CS: embedded_hal::digital::OutputPin,
{
    spi: SPI,
    cs: CS,
}

#[derive(Debug, Clone)]
pub enum SdMmcError {
    Transport,
    CantEnableCRC
}

impl<SPI, CS> SdMmcSpi<SPI, CS>
where
    SPI: embedded_hal::spi::FullDuplex<u8>,
    CS: embedded_hal::digital::OutputPin,
    <SPI as embedded_hal::spi::FullDuplex<u8>>::Error: core::fmt::Debug,
{
    pub fn new(spi: SPI, cs: CS) -> SdMmcSpi<SPI, CS> {
        SdMmcSpi { spi, cs }
    }

    /// This routine must be performed with an SPI clock speed of around 100 - 400 kHz.
    /// Afterwards you may increase the SPI clock speed.
    pub fn init(&mut self) -> Result<(), Error<SdMmcSpi<SPI, CS>>> {
        self.cs.set_high();
        // Supply minimum of 74 clock cycles
        for _ in 0..10 {
            block!(self.spi.send(0xFF)).map_err(|_e| Error::DeviceError(SdMmcError::Transport))?;
        }
        self.cs.set_low();
        while self.card_command(CMD0, 0) != R1_IDLE_STATE {
            // Check for timeout
        }
        if self.card_command(CMD59, 1) != R1_IDLE_STATE {
            return Err(Error::DeviceError(SdMmcError::CantEnableCRC))
        }
        Ok(())
    }

    pub fn card_command(&mut self, _command: u8, _arg: u8) -> u8 {
        unimplemented!()
    }

    pub fn card_size(&mut self) -> Result<BlockIdx, Error<SdMmcSpi<SPI, CS>>> {
        unimplemented!()
    }
    pub fn erase(&mut self, _first_block: BlockIdx, _last_block: BlockIdx) -> Result<(), Error<SdMmcSpi<SPI, CS>>> {
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
