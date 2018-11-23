//! embedded-sdmmc-rs - FAT file system
//!
//! Implements the File Allocation Table file system

use super::{Block, BlockDevice, BlockIdx, Controller, Error};
use byteorder::{ByteOrder, LittleEndian};

/// Marker for a FAT32 partition. Sometimes also use for FAT16 formatted
/// partitions.
pub const PARTITION_ID_FAT32_LBA: u8 = 0x0C;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FatType {
    Fat32,
    Fat16
}

/// Identifies a FAT16/32 Volume on the disk.
#[derive(PartialEq, Eq)]
pub struct Volume {
    pub(crate) lba_start: BlockIdx,
    pub(crate) num_blocks: BlockIdx,
    pub(crate) name: [u8; 11],
    pub(crate) fat_type: FatType,
}

impl core::fmt::Debug for Volume {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(fmt, "Volume(")?;
        match core::str::from_utf8(&self.name) {
            Ok(s) => write!(fmt, "name={:?}, ", s)?,
            Err(_e) => write!(fmt, "raw_name={:?}, ", &self.name)?,
        }
        write!(fmt, "lba_start=0x{:08x}, ", self.lba_start.0)?;
        write!(fmt, "num_blocks=0x{:08x}, ", self.num_blocks.0)?;
        write!(fmt, "type=FAT{}", match self.fat_type {
            FatType::Fat16 => 16,
            FatType::Fat32 => 32,
        })?;
        Ok(())
    }
}


// Example FAT32 info block:
// eb3c906d6b66732e66617400021001000200020000f820003f00ff0000000000
// 00e00100800129bbb07177626f6f742020202020202046415431362020200e1f
// be5b7cac22c0740b56b40ebb0700cd105eebf032e4cd16cd19ebfe5468697320
// 6973206e6f74206120626f6f7461626c65206469736b2e2020506c6561736520
// 696e73657274206120626f6f7461626c6520666c6f70707920616e640d0a7072
// 65737320616e79206b657920746f2074727920616761696e202e2e2e200d0a00
// 0000000000000000000000000000000000000000000000000000000000000000
// 0000000000000000000000000000000000000000000000000000000000000000
// 0000000000000000000000000000000000000000000000000000000000000000
// 0000000000000000000000000000000000000000000000000000000000000000
// 0000000000000000000000000000000000000000000000000000000000000000
// 0000000000000000000000000000000000000000000000000000000000000000
// 0000000000000000000000000000000000000000000000000000000000000000
// 0000000000000000000000000000000000000000000000000000000000000000
// 0000000000000000000000000000000000000000000000000000000000000000
// 00000000000000000000000000000000000000000000000000000000000055aa


pub fn parse_volume<D>(
    controller: &mut Controller<D>,
    lba_start: BlockIdx,
    num_blocks: BlockIdx,
) -> Result<Volume, Error<D::Error>>
where
    D: BlockDevice,
    D::Error: core::fmt::Debug,
{
    const FOOTER_START: usize = 510;
    const FOOTER_VALUE: u16 = 0xAA55;

    let mut blocks = [Block::new()];
    controller
        .block_device
        .read(&mut blocks, lba_start)
        .map_err(|e| Error::DeviceError(e))?;
    let block = &blocks[0];
    if LittleEndian::read_u16(&block[FOOTER_START..FOOTER_START + 2]) != FOOTER_VALUE {
        return Err(Error::FormatError("Invalid partition signature."));
    }
    let mut volume = Volume {
        lba_start,
        num_blocks,
        name: [0u8; 11],
        fat_type: FatType::Fat32
    };
    let bpb_fatsz16 = block[22];
    if bpb_fatsz16 != 0 {
        volume.fat_type = FatType::Fat16;
        volume.name[..].copy_from_slice(&block[43..54]);
    } else {
        volume.name[..].copy_from_slice(&block[71..82]);
    }
    Ok(volume)
}
