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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockIdx(pub u32);

/// Represents a block device which is <= 2 TiB in size.
pub trait BlockDevice {
    type Error: core::fmt::Debug;
    /// Read one or more blocks, starting at the given block index.
    fn read(&mut self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Self::Error>;
    /// Write one or more blocks, starting at the given block index.
    fn write(&mut self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error>;
}

impl core::ops::Deref for Block {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.contents
    }
}

impl core::fmt::Debug for Block {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        writeln!(fmt, "Block:")?;
        for line in self.contents.chunks(32) {
            for b in line {
                write!(fmt, "{:02x}", b)?;
            }
            write!(fmt, " ")?;
            for &b in line {
                if b >= 0x20 && b <= 0x7F {
                    write!(fmt, "{}", b as char)?;
                } else {
                    write!(fmt, ".")?;
                }
            }
            write!(fmt, "\n")?;
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

impl BlockIdx {
    pub fn into_bytes(self) -> u64 {
        (self.0 as u64) * (Block::LEN as u64)
    }
}
