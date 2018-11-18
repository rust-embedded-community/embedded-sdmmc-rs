//! embedded-sdmmc-rs - SDMMC Protocol
//!
//! Implements the SD/MMC protocol on some generic SPI interface.

use super::{Block, BlockDevice, BlockIdx};

use super::Error as GenericError;

type Error<S> = GenericError<SdMmcSpi<S>>;

pub struct SdMmcSpi<S>
where
    S: embedded_hal::spi::FullDuplex<u8>,
{
    spi: S,
}

#[derive(Debug, Clone)]
pub enum SdError {
    Unknown,
}

impl<S> SdMmcSpi<S>
where
    S: embedded_hal::spi::FullDuplex<u8>,
{
    pub fn new(spi: S) -> SdMmcSpi<S> {
        SdMmcSpi { spi }
    }
    pub fn init(&mut self) -> Result<(), Error<S>> {
        unimplemented!()
    }
    pub fn card_size(&mut self) -> Result<BlockIdx, Error<S>> {
        unimplemented!()
    }
    pub fn erase(&mut self, _first_block: BlockIdx, _last_block: BlockIdx) -> Result<(), Error<S>> {
        unimplemented!()
    }
}

impl<S> BlockDevice for SdMmcSpi<S>
where
    S: embedded_hal::spi::FullDuplex<u8>,
{
    type Error = SdError;

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
