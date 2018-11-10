//! embedded-sdmmc: A SD/MMC Library written in Embedded Rust

#![no_std]

use byteorder::{ByteOrder, LittleEndian};

/// Represents a standard 512 byte block/sector.
#[derive(Clone)]
pub struct Block {
    pub contents: [u8; Block::LEN],
}

/// Represents a block device which is <= 2 TiB in size.
pub trait BlockDevice {
    type Error;
    fn read(&mut self, blocks: &mut [Block], start_block_idx: u32) -> Result<(), Self::Error>;
    fn write(&mut self, blocks: &[Block], start_block_idx: u32) -> Result<(), Self::Error>;
}

#[derive(Debug, Copy, Clone)]
pub enum Error<D> where D: BlockDevice {
    DeviceError(D::Error),
    FormatError(&'static str),
    NoSuchVolume,
}

pub struct Controller<D> where D: BlockDevice {
    pub block_device: D
}

pub struct Card {
    _x: (),
}

/// Identifies a FAT32 Volume on the disk.
pub struct Volume {
    lba_start: u32,
    num_blocks: u32,
    name: [u8; 11],
}

pub struct Directory<'a> {
    _parent: &'a Volume,
}

pub struct DirEntry {
    pub name: [u8; 11],
    pub mtine: u32,
    pub ctime: u32,
    pub attributes: u8,
}

pub struct File<'a> {
    _parent: &'a Volume,
    _offset: u32,
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

impl<D> Controller<D> where D: BlockDevice {
    pub const PARTITION_ID_FAT32_LBA: u8 = 0x0C;

    pub fn new(block_device: D) -> Controller<D> {
        Controller {
            block_device
        }
    }

    pub fn get_volume(&mut self, volume_idx: usize) -> Result<Volume, Error<D>> {
        let mut blocks = [Block::new()];
        let (lba_start, num_blocks) = {
            self.block_device.read(&mut blocks, 0).map_err(|e| Error::DeviceError(e))?;
            let block = &blocks[0];
            if block[511] != 0xAA {
                return Err(Error::FormatError("Invalid MBR signature."));
            }
            if block[510] != 0x55 {
                return Err(Error::FormatError("Invalid MBR signature."));
            }
            let partition = match volume_idx {
                0 => {
                    &block[446..462]
                }
                1 => {
                    &block[462..478]
                }
                2 => {
                    &block[478..492]
                }
                3 => {
                    &block[492..510]
                }
                _ => {
                    return Err(Error::NoSuchVolume);
                }
            };
            if (partition[0] & 0x7F) != 0x00 {
                return Err(Error::FormatError("Invalid partition status."));
            }
            if partition[4] != Self::PARTITION_ID_FAT32_LBA {
                return Err(Error::FormatError("Partition is not of type FAT32 LBA."));
            }
            (
                LittleEndian::read_u32(&partition[8..12]),
                LittleEndian::read_u32(&partition[12..16]),
            )
        };
        let volume = {
            self.block_device.read(&mut blocks, lba_start).map_err(|e| Error::DeviceError(e))?;
            let block = &blocks[0];
            if block[511] != 0xAA {
                return Err(Error::FormatError("Invalid partition signature."));
            }
            if block[510] != 0x55 {
                return Err(Error::FormatError("Invalid partition signature."));
            }
            let mut volume = Volume {
                lba_start,
                num_blocks,
                name: [0u8; 11]
            };
            volume.name[..].copy_from_slice(&block[71..82]);
            volume
        };
        Ok(volume)
    }
}

impl core::fmt::Debug for Volume {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(fmt, "Volume(name={:?}, ", core::str::from_utf8(&self.name))?;
        write!(fmt, "lba_start=0x{:08x}, ", self.lba_start)?;
        write!(fmt, "num_blocks=0x{:08x})", self.num_blocks)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
