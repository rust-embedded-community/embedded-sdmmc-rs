//! Traits and types for working with Block Devices.
//!
//! Generic code for handling block devices, such as types for identifying
//! a particular block on a block device by its index.

use embedded_storage::block::{BlockCount, BlockDevice, BlockIdx};

/// A standard 512 byte block (also known as a sector).
///
/// IBM PC formatted 5.25" and 3.5" floppy disks, IDE/SATA Hard Drives up to
/// about 2 TiB, and almost all SD/MMC cards have 512 byte blocks.
///
/// This library does not support devices with a block size other than 512
/// bytes.
pub type Block = [u8; BLOCK_LEN];

/// All our blocks are a fixed length of 512 bytes. We do not support
/// 'Advanced Format' Hard Drives with 4 KiB blocks, nor weird old
/// pre-3.5-inch floppy disk formats.
pub const BLOCK_LEN: usize = 512;

/// Sometimes we want `LEN` as a `u32` and the casts don't look nice.
pub const BLOCK_LEN_U32: u32 = 512;

/// Sometimes we want `LEN` as a `u64` and the casts don't look nice.
pub const BLOCK_LEN_U64: u64 = 512;

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
            block: [[0; BLOCK_LEN]],
            block_idx: None,
        }
    }

    /// Read a block, and return a reference to it.
    pub fn read(&mut self, block_idx: BlockIdx) -> Result<&Block, D::Error> {
        if self.block_idx != Some(block_idx) {
            self.block_idx = None;
            self.block_device.read(block_idx, &mut self.block)?;
            self.block_idx = Some(block_idx);
        }
        Ok(&self.block[0])
    }

    /// Read a block, and return a reference to it.
    pub fn read_mut(&mut self, block_idx: BlockIdx) -> Result<&mut Block, D::Error> {
        if self.block_idx != Some(block_idx) {
            self.block_idx = None;
            self.block_device.read(block_idx, &mut self.block)?;
            self.block_idx = Some(block_idx);
        }
        Ok(&mut self.block[0])
    }

    /// Write back a block you read with [`Self::read_mut`] and then modified.
    pub fn write_back(&mut self) -> Result<(), D::Error> {
        self.block_device.write(
            self.block_idx.expect("write_back with no read"),
            &self.block,
        )
    }

    /// Access a blank sector
    pub fn blank_mut(&mut self, block_idx: BlockIdx) -> &mut Block {
        self.block_idx = Some(block_idx);
        for b in self.block[0].iter_mut() {
            *b = 0;
        }
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

/// Convert a block index into a 64-bit byte offset from the start of the
/// volume. Useful if your underlying block device actually works in
/// bytes, like `open("/dev/mmcblk0")` does on Linux.
pub fn block_index_into_bytes(block_index: BlockIdx) -> u64 {
    block_index.0 * BLOCK_LEN_U64
}

/// How many blocks are required to hold this many bytes.
///
/// ```
/// # use embedded_sdmmc::blockdevice::{block_count_from_bytes};
/// # use embedded_storage::block::BlockCount;
/// assert_eq!(block_count_from_bytes(511), BlockCount(1));
/// assert_eq!(block_count_from_bytes(512), BlockCount(1));
/// assert_eq!(block_count_from_bytes(513), BlockCount(2));
/// assert_eq!(block_count_from_bytes(1024), BlockCount(2));
/// assert_eq!(block_count_from_bytes(1025), BlockCount(3));
/// ```
pub const fn block_count_from_bytes(byte_count: u64) -> BlockCount {
    let mut count = byte_count / BLOCK_LEN_U64;
    if (count * BLOCK_LEN_U64) != byte_count {
        count += 1;
    }
    BlockCount(count)
}

/// Take a number of blocks and increment by the integer number of blocks
/// required to get to the block that holds the byte at the given offset.
pub fn block_count_offset_bytes(base: BlockCount, offset: u64) -> BlockCount {
    BlockCount(base.0 + (offset / BLOCK_LEN_U64))
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
