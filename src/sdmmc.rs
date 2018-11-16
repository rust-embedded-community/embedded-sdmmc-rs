//! embedded-sdmmc-rs - SDMMC Protocol
//!
//! Implements the SD/MMC protocol on some generic SPI interface.

use super::{Block, BlockDevice, BlockIdx};

use super::Error as GenericError;

type Error = GenericError<SdMmcDevice>;

pub struct SdMmcDevice();

pub enum SdError {
    Unknown,
}

impl SdMmcDevice {
    pub fn new(spi: ()) -> Result<SdMmcDevice, Error> {
        unimplemented!()
    }
    pub fn init(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
    pub fn cardSize(&mut self) -> Result<BlockIdx, Error> {
        unimplemented!()
    }
    pub fn erase(&mut self, first_block: BlockIdx, last_block: BlockIdx) -> Result<(), Error> {
        unimplemented!()
    }
}

impl BlockDevice for SdMmcDevice {
    type Error = SdError;

    /// Read one or more blocks, starting at the given block index.
    fn read(&mut self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        unimplemented!();
    }
    /// Write one or more blocks, starting at the given block index.
    fn write(&mut self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        unimplemented!();
    }
    /// Complete a multi-block transaction and return the SD card to idle mode.
    fn sync(&mut self) -> Result<(), Self::Error> {
        unimplemented!();
    }
}
