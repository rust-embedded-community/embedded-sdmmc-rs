//! embedded-sdmmc: A SD/MMC Library written in Embedded Rust

#![no_std]

use byteorder::{ByteOrder, LittleEndian};

pub mod blockdevice;
pub mod fat;
pub mod filesystem;
pub mod sdmmc;

pub use crate::blockdevice::{Block, BlockDevice};
pub use crate::fat::Volume as FatVolume;
pub use crate::filesystem::{DirEntry, Directory, File};

#[derive(Debug, Copy, Clone)]
pub enum Error<D> where D: BlockDevice {
    DeviceError(D::Error),
    FormatError(&'static str),
    NoSuchVolume,
}

/// A `Controller` wraps a block device and gives access to the volumes within it.
pub struct Controller<D> where D: BlockDevice {
    pub block_device: D
}

#[derive(Debug)]
pub enum Volume {
    Fat(FatVolume)
}

impl<D> Controller<D> where D: BlockDevice {

    pub fn new(block_device: D) -> Controller<D> {
        Controller {
            block_device
        }
    }

    pub fn get_volume(&mut self, volume_idx: usize) -> Result<Volume, Error<D>> {
        const PARTITION1_START: usize = 446;
        const PARTITION2_START: usize = 462;
        const PARTITION3_START: usize = 478;
        const PARTITION4_START: usize = 492;
        const FOOTER_START: usize = 510;
        const FOOTER_VALUE: u16 = 0xAA55;
        const PARTITION_INFO_LENGTH: usize = 16;
        const PARTITION_INFO_STATUS_INDEX: usize = 0;
        const PARTITION_INFO_TYPE_INDEX: usize = 4;

        let (lba_start, num_blocks) = {
            let mut blocks = [Block::new()];
            self.block_device.read(&mut blocks, 0).map_err(|e| Error::DeviceError(e))?;
            let block = &blocks[0];
            // We only support Master Boot Record (MBR) partitioned cards, not
            // GUID Partition Table (GPT)
            if LittleEndian::read_u16(&block[FOOTER_START..FOOTER_START+2]) != FOOTER_VALUE {
                return Err(Error::FormatError("Invalid MBR signature."));
            }
            let partition = match volume_idx {
                0 => {
                    &block[PARTITION1_START..(PARTITION1_START+PARTITION_INFO_LENGTH)]
                }
                1 => {
                    &block[PARTITION2_START..(PARTITION2_START+PARTITION_INFO_LENGTH)]
                }
                2 => {
                    &block[PARTITION3_START..(PARTITION3_START+PARTITION_INFO_LENGTH)]
                }
                3 => {
                    &block[PARTITION4_START..(PARTITION4_START+PARTITION_INFO_LENGTH)]
                }
                _ => {
                    return Err(Error::NoSuchVolume);
                }
            };
            // Only 0x80 and 0x00 are value (bootable, and non-bootable)
            if (partition[PARTITION_INFO_STATUS_INDEX] & 0x7F) != 0x00 {
                return Err(Error::FormatError("Invalid partition status."));
            }
            // We only handle FAT32 LBA for now
            if partition[PARTITION_INFO_TYPE_INDEX] != fat::PARTITION_ID_FAT32_LBA {
                return Err(Error::FormatError("Partition is not of type FAT32 LBA."));
            }
            let lba_start = LittleEndian::read_u32(&partition[8..12]);
            let num_blocks = LittleEndian::read_u32(&partition[12..16]);
            (lba_start, num_blocks)
        };
        let volume = fat::parse_volume(self, lba_start, num_blocks)?;
        Ok(Volume::Fat(volume))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
