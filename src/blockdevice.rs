//! embedded-sdmmc-rs - Block Devices
//!
//! Generic code for handling block devices.

/// Represents a standard 512 byte block (also known as a sector). IBM PC
/// formatted 5.25" and 3.5" floppy disks, SD/MMC cards up to 1 GiB in size
/// and IDE/SATA Hard Drives up to about 2 TiB all have 512 byte blocks.
///
/// This library does not support devices with a block size other than 512
/// bytes.
#[derive(Clone)]
pub struct Block {
    pub contents: [u8; Block::LEN],
}

/// Represents a block device which is <= 2 TiB in size.
pub trait BlockDevice {
    type Error;
    /// Read one or more blocks, starting at the given block index.
    fn read(&mut self, blocks: &mut [Block], start_block_idx: u32) -> Result<(), Self::Error>;
    /// Write one or more blocks, starting at the given block index.
    fn write(&mut self, blocks: &[Block], start_block_idx: u32) -> Result<(), Self::Error>;
    /// Complete a multi-block transaction and return the SD card to idle mode.
    fn sync(&mut self) -> Result<(), Self::Error>;
}

impl core::ops::Deref for Block {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.contents
    }
}

impl core::fmt::Debug for Block {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(fmt, "Block: ")?;
        for b in self.contents.iter() {
            write!(fmt, "{:02x} ", b)?;
        }
        Ok(())
    }
}

impl Block {
    pub const LEN: usize = 512;

    pub fn new() -> Block {
        Block {
            contents: [0u8; Self::LEN],
        }
    }
}
