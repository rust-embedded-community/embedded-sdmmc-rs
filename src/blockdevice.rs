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
    /// The 512 bytes in this block (or sector).
    pub contents: [u8; Block::LEN],
}

/// Represents the linear numeric address of a block (or sector). The first
/// block on a disk gets `BlockIdx(0)` (which usually contains the Master Boot
/// Record).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockIdx(pub u32);

/// Represents the a number of blocks (or sectors). Add this to a `BlockIdx`
/// to get an actual address on disk.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockCount(pub u32);

impl core::ops::Add<BlockCount> for BlockIdx {
    type Output = BlockIdx;
    fn add(self, rhs: BlockCount) -> BlockIdx {
        BlockIdx(self.0 + rhs.0)
    }
}

impl core::ops::Add<BlockCount> for BlockCount {
    type Output = BlockCount;
    fn add(self, rhs: BlockCount) -> BlockCount {
        BlockCount(self.0 + rhs.0)
    }
}

impl BlockIdx {
    /// Create an iterator from the current `BlockIdx` through the given
    /// number of blocks.
    pub fn range(&self, num: BlockCount) -> BlockIter {
        BlockIter::new(*self, *self + num)
    }
}

pub struct BlockIter {
    inclusive_end: BlockIdx,
    current: BlockIdx
}

impl BlockIter {
    pub fn new(start: BlockIdx, inclusive_end: BlockIdx) -> BlockIter {
        BlockIter {
            inclusive_end,
            current: start,
        }
    }
}

impl core::iter::Iterator for BlockIter {
    type Item = BlockIdx;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current.0 >= self.inclusive_end.0 {
            None
        } else {
            let this = self.current;
            self.current = self.current + BlockCount(1);
            Some(this)
        }
    }
}

/// Represents a block device - a device which can read and write blocks (or
/// sectors). Only supports devices which are <= 2 TiB in size.
pub trait BlockDevice {
    /// The errors that the `BlockDevice` can return. Must be debug formattable.
    type Error: core::fmt::Debug;
    /// Read one or more blocks, starting at the given block index.
    fn read(&self, blocks: &mut [Block], start_block_idx: BlockIdx, reason: &str) -> Result<(), Self::Error>;
    /// Write one or more blocks, starting at the given block index.
    fn write(&mut self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error>;
    /// Determine how many blocks this device can hold.
    fn num_blocks(&self) -> Result<BlockIdx, Self::Error>;
}

impl core::ops::Deref for Block {
    type Target = [u8; 512];
    fn deref(&self) -> &[u8; 512] {
        &self.contents
    }
}

impl core::ops::DerefMut for Block {
    fn deref_mut(&mut self) -> &mut [u8; 512] {
        &mut self.contents
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
    /// All our blocks are a fixed length of 512 bytes. We do not support
    /// 'Advanced Format' Hard Drives with 4 KiB blocks, nor weird old
    /// pre-3.5-inch floppy disk formats.
    pub const LEN: usize = 512;

    /// Create a new block full of zeros.
    pub fn new() -> Block {
        Block {
            contents: [0u8; Self::LEN],
        }
    }
}

impl BlockIdx {
    /// Convert a block index into a 64-bit byte offset from the start of the
    /// volume. Useful if your underlying block device actually works in
    /// bytes, like `open("/dev/mmcblk0")` does on Linux.
    pub fn into_bytes(self) -> u64 {
        (self.0 as u64) * (Block::LEN as u64)
    }
}
