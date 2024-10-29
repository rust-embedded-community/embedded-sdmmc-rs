//! Traits and types for working with Block Devices.
//!
//! Generic code for handling block devices, such as types for identifying
//! a particular block on a block device by its index.

/// A standard 512 byte block (also known as a sector).
///
/// IBM PC formatted 5.25" and 3.5" floppy disks, IDE/SATA Hard Drives up to
/// about 2 TiB, and almost all SD/MMC cards have 512 byte blocks.
///
/// This library does not support devices with a block size other than 512
/// bytes.
#[derive(Clone)]
pub struct Block {
    /// The 512 bytes in this block (or sector).
    pub contents: [u8; Block::LEN],
}

impl Block {
    /// All our blocks are a fixed length of 512 bytes. We do not support
    /// 'Advanced Format' Hard Drives with 4 KiB blocks, nor weird old
    /// pre-3.5-inch floppy disk formats.
    pub const LEN: usize = 512;

    /// Sometimes we want `LEN` as a `u32` and the casts don't look nice.
    pub const LEN_U32: u32 = 512;

    /// Create a new block full of zeros.
    pub fn new() -> Block {
        Block {
            contents: [0u8; Self::LEN],
        }
    }
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
                if (0x20..=0x7F).contains(&b) {
                    write!(fmt, "{}", b as char)?;
                } else {
                    write!(fmt, ".")?;
                }
            }
            writeln!(fmt)?;
        }
        Ok(())
    }
}

impl Default for Block {
    fn default() -> Self {
        Self::new()
    }
}

/// A block device - a device which can read and write blocks (or
/// sectors). Only supports devices which are <= 2 TiB in size.
pub trait BlockDevice {
    /// The errors that the `BlockDevice` can return. Must be debug formattable.
    type Error: core::fmt::Debug;
    /// Read one or more blocks, starting at the given block index.
    fn read(&self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Self::Error>;
    /// Write one or more blocks, starting at the given block index.
    fn write(&self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error>;
    /// Determine how many blocks this device can hold.
    fn num_blocks(&self) -> Result<BlockCount, Self::Error>;
}

/// A caching layer for block devices
///
/// Caches a single block.
#[derive(Debug)]
pub struct BlockCache<D> {
    block_device: D,
    block: [Block; 1],
    block_idx: Option<BlockIdx>,
}

impl<D> BlockCache<D>
where
    D: BlockDevice,
{
    /// Create a new block cache
    pub fn new(block_device: D) -> BlockCache<D> {
        BlockCache {
            block_device,
            block: [Block::new()],
            block_idx: None,
        }
    }

    /// Read a block, and return a reference to it.
    pub fn read(&mut self, block_idx: BlockIdx) -> Result<&Block, D::Error> {
        if self.block_idx != Some(block_idx) {
            self.block_idx = None;
            self.block_device.read(&mut self.block, block_idx)?;
            self.block_idx = Some(block_idx);
        }
        Ok(&self.block[0])
    }

    /// Read a block, and return a reference to it.
    pub fn read_mut(&mut self, block_idx: BlockIdx) -> Result<&mut Block, D::Error> {
        if self.block_idx != Some(block_idx) {
            self.block_idx = None;
            self.block_device.read(&mut self.block, block_idx)?;
            self.block_idx = Some(block_idx);
        }
        Ok(&mut self.block[0])
    }

    /// Write back a block you read with [`Self::read_mut`] and then modified.
    pub fn write_back(&mut self) -> Result<(), D::Error> {
        self.block_device.write(
            &self.block,
            self.block_idx.expect("write_back with no read"),
        )
    }

    /// Access a blank sector
    pub fn blank_mut(&mut self, block_idx: BlockIdx) -> &mut Block {
        self.block_idx = Some(block_idx);
        self.block[0].fill(0);
        &mut self.block[0]
    }

    /// Access the block device
    pub fn block_device(&mut self) -> &mut D {
        // invalidate the cache
        self.block_idx = None;
        // give them the block device
        &mut self.block_device
    }

    /// Get the block device back
    pub fn free(self) -> D {
        self.block_device
    }
}

/// The linear numeric address of a block (or sector).
///
/// The first block on a disk gets `BlockIdx(0)` (which usually contains the
/// Master Boot Record).
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockIdx(pub u32);

impl BlockIdx {
    /// Convert a block index into a 64-bit byte offset from the start of the
    /// volume. Useful if your underlying block device actually works in
    /// bytes, like `open("/dev/mmcblk0")` does on Linux.
    pub fn into_bytes(self) -> u64 {
        (u64::from(self.0)) * (Block::LEN as u64)
    }

    /// Create an iterator from the current `BlockIdx` through the given
    /// number of blocks.
    pub fn range(self, num: BlockCount) -> BlockIter {
        BlockIter::new(self, self + BlockCount(num.0))
    }
}

impl core::ops::Add<BlockCount> for BlockIdx {
    type Output = BlockIdx;
    fn add(self, rhs: BlockCount) -> BlockIdx {
        BlockIdx(self.0 + rhs.0)
    }
}

impl core::ops::AddAssign<BlockCount> for BlockIdx {
    fn add_assign(&mut self, rhs: BlockCount) {
        self.0 += rhs.0
    }
}

impl core::ops::Sub<BlockCount> for BlockIdx {
    type Output = BlockIdx;
    fn sub(self, rhs: BlockCount) -> BlockIdx {
        BlockIdx(self.0 - rhs.0)
    }
}

impl core::ops::SubAssign<BlockCount> for BlockIdx {
    fn sub_assign(&mut self, rhs: BlockCount) {
        self.0 -= rhs.0
    }
}

/// The a number of blocks (or sectors).
///
/// Add this to a `BlockIdx` to get an actual address on disk.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockCount(pub u32);

impl core::ops::Add<BlockCount> for BlockCount {
    type Output = BlockCount;
    fn add(self, rhs: BlockCount) -> BlockCount {
        BlockCount(self.0 + rhs.0)
    }
}

impl core::ops::AddAssign<BlockCount> for BlockCount {
    fn add_assign(&mut self, rhs: BlockCount) {
        self.0 += rhs.0
    }
}

impl core::ops::Sub<BlockCount> for BlockCount {
    type Output = BlockCount;
    fn sub(self, rhs: BlockCount) -> BlockCount {
        BlockCount(self.0 - rhs.0)
    }
}

impl core::ops::SubAssign<BlockCount> for BlockCount {
    fn sub_assign(&mut self, rhs: BlockCount) {
        self.0 -= rhs.0
    }
}

impl BlockCount {
    /// How many blocks are required to hold this many bytes.
    ///
    /// ```
    /// # use embedded_sdmmc::BlockCount;
    /// assert_eq!(BlockCount::from_bytes(511), BlockCount(1));
    /// assert_eq!(BlockCount::from_bytes(512), BlockCount(1));
    /// assert_eq!(BlockCount::from_bytes(513), BlockCount(2));
    /// assert_eq!(BlockCount::from_bytes(1024), BlockCount(2));
    /// assert_eq!(BlockCount::from_bytes(1025), BlockCount(3));
    /// ```
    pub const fn from_bytes(byte_count: u32) -> BlockCount {
        let mut count = byte_count / Block::LEN_U32;
        if (count * Block::LEN_U32) != byte_count {
            count += 1;
        }
        BlockCount(count)
    }

    /// Take a number of blocks and increment by the integer number of blocks
    /// required to get to the block that holds the byte at the given offset.
    pub fn offset_bytes(self, offset: u32) -> Self {
        BlockCount(self.0 + (offset / Block::LEN_U32))
    }
}

/// An iterator returned from `Block::range`.
pub struct BlockIter {
    inclusive_end: BlockIdx,
    current: BlockIdx,
}

impl BlockIter {
    /// Create a new `BlockIter`, from the given start block, through (and
    /// including) the given end block.
    pub const fn new(start: BlockIdx, inclusive_end: BlockIdx) -> BlockIter {
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
            self.current += BlockCount(1);
            Some(this)
        }
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
