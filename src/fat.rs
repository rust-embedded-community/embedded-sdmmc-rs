//! embedded-sdmmc-rs - FAT file system
//!
//! Implements the File Allocation Table file system. Supports FAT16 and FAT32 volumes.
#![allow(unused)]

use byteorder::{ByteOrder, LittleEndian};
use crate::{
    Attributes, Block, BlockDevice, BlockIdx, Controller, DirEntry, Directory, Error, Inode,
    ShortFileName, TimeSource, Timestamp, VolumeType,
};

/// Identifies a FAT16 Volume on the disk.
#[derive(PartialEq, Eq)]
pub struct Fat16Volume {
    pub(crate) lba_start: BlockIdx,
    pub(crate) num_blocks: BlockIdx,
    pub(crate) name: [u8; 11],
}

/// Identifies a FAT32 Volume on the disk.
#[derive(PartialEq, Eq)]
pub struct Fat32Volume {
    pub(crate) lba_start: BlockIdx,
    pub(crate) num_blocks: BlockIdx,
    pub(crate) name: [u8; 11],
}

impl core::fmt::Debug for Fat16Volume {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(fmt, "Volume(")?;
        match core::str::from_utf8(&self.name) {
            Ok(s) => write!(fmt, "name={:?}, ", s)?,
            Err(_e) => write!(fmt, "raw_name={:?}, ", &self.name)?,
        }
        write!(fmt, "lba_start=0x{:08x}, ", self.lba_start.0)?;
        write!(fmt, "num_blocks=0x{:08x}, ", self.num_blocks.0)?;
        write!(fmt, "type=FAT16)")?;
        Ok(())
    }
}

impl core::fmt::Debug for Fat32Volume {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(fmt, "Volume(")?;
        match core::str::from_utf8(&self.name) {
            Ok(s) => write!(fmt, "name={:?}, ", s)?,
            Err(_e) => write!(fmt, "raw_name={:?}, ", &self.name)?,
        }
        write!(fmt, "lba_start=0x{:08x}, ", self.lba_start.0)?;
        write!(fmt, "num_blocks=0x{:08x}, ", self.num_blocks.0)?;
        write!(fmt, "type=FAT32)")?;
        Ok(())
    }
}

struct Bpb<'a> {
    data: &'a [u8; 512],
    fat16: bool,
}

impl<'a> Bpb<'a> {
    const FOOTER_VALUE: u16 = 0xAA55;

    pub fn new(data: &[u8; 512]) -> Result<Bpb, &'static str> {
        let mut bpb = Bpb { data, fat16: false };
        if bpb.footer() != Self::FOOTER_VALUE {
            return Err("Bad BPB footer");
        }

        let root_dir_sectors =
            ((bpb.root_entries_count() as u32 * 32) + (Block::LEN as u32 - 1)) / Block::LEN as u32;
        let fat_size = bpb.fat_size();
        let tot_sec = bpb.total_sectors();
        let data_sectors = tot_sec
            - (bpb.reserved_sector_count() as u32
                + (bpb.num_fats() as u32 * fat_size)
                + root_dir_sectors);
        let cluster_count = data_sectors / bpb.sectors_per_cluster() as u32;
        if cluster_count < 4085 {
            return Err("FAT12 is unsupported");
        } else if cluster_count < 65525 {
            bpb.fat16 = true;
        } else {
            bpb.fat16 = false;
        }

        if bpb.fat16 {
            // FAT16 has no version
            Ok(bpb)
        } else if bpb.fs_ver() == 0 {
            // Only support FAT32 version 0.0
            Ok(bpb)
        } else {
            Err("Invalid FAT format")
        }
    }

    // FAT16/FAT32
    define_field!(bytes_per_sector, u16, 11);
    define_field!(sectors_per_cluster, u8, 13);
    define_field!(reserved_sector_count, u16, 14);
    define_field!(num_fats, u8, 16);
    define_field!(root_entries_count, u16, 17);
    define_field!(total_sectors16, u16, 19);
    define_field!(media, u8, 21);
    define_field!(fat_size16, u16, 22);
    define_field!(sectors_per_track, u16, 24);
    define_field!(num_heads, u16, 26);
    define_field!(hidden_sectors, u32, 28);
    define_field!(total_sectors32, u32, 32);
    define_field!(footer, u16, 510);

    // FAT32 only
    define_field!(fat_size32, u32, 36);
    define_field!(fs_ver, u16, 42);
    define_field!(fs_info, u16, 48);
    define_field!(backup_boot_sector, u16, 50);

    pub fn oem_name(&self) -> &[u8] {
        &self.data[3..11]
    }

    // FAT16/FAT32 functions

    pub fn drive_number(&self) -> u8 {
        if self.fat16 {
            self.data[36]
        } else {
            self.data[64]
        }
    }

    pub fn boot_signature(&self) -> u8 {
        if self.fat16 {
            self.data[38]
        } else {
            self.data[66]
        }
    }

    pub fn volume_id(&self) -> u32 {
        if self.fat16 {
            LittleEndian::read_u32(&self.data[39..=42])
        } else {
            LittleEndian::read_u32(&self.data[67..=70])
        }
    }

    pub fn volume_label(&self) -> &[u8] {
        if self.fat16 {
            &self.data[43..=53]
        } else {
            &self.data[71..=81]
        }
    }

    pub fn fs_type(&self) -> &[u8] {
        if self.fat16 {
            &self.data[54..=61]
        } else {
            &self.data[82..=89]
        }
    }

    // FAT32 only functions

    pub fn current_fat(&self) -> u8 {
        self.data[40] & 0x0F
    }

    pub fn use_specific_fat(&self) -> bool {
        (self.data[40] & 0x80) != 0x00
    }

    pub fn root_cluster(&self) -> Inode {
        Inode(LittleEndian::read_u32(&self.data[44..=47]))
    }

    // Magic functions that get the right FAT16/FAT32 result

    pub fn fat_size(&self) -> u32 {
        let result = self.fat_size16() as u32;
        if result != 0 {
            result
        } else {
            self.fat_size32()
        }
    }

    pub fn total_sectors(&self) -> u32 {
        let result = self.total_sectors16() as u32;
        if result != 0 {
            result
        } else {
            self.total_sectors32()
        }
    }
}

struct OnDiskDirEntry<'a> {
    data: &'a [u8],
}

impl<'a> OnDiskDirEntry<'a> {
    const LEN: usize = 32;

    define_field!(raw_attr, u8, 11);
    define_field!(create_time, u16, 14);
    define_field!(create_date, u16, 16);
    define_field!(last_access_data, u16, 18);
    define_field!(first_cluster_hi, u16, 20);
    define_field!(write_time, u16, 22);
    define_field!(write_date, u16, 24);
    define_field!(first_cluster_lo, u16, 26);
    define_field!(file_size, u32, 28);

    fn new(data: &[u8]) -> OnDiskDirEntry {
        OnDiskDirEntry { data }
    }

    fn is_end(&self) -> bool {
        self.data[0] == 0x00
    }

    fn is_valid(&self) -> bool {
        !self.is_end() && (self.data[0] != 0xE5)
    }

    fn is_lfn(&self) -> bool {
        let attributes = Attributes::create_from_fat(self.raw_attr());
        attributes.is_lfn()
    }

    fn matches(&self, sfn: &ShortFileName) -> bool {
        self.data[0..11] == sfn.contents
    }

    fn first_cluster(&self) -> Inode {
        let cluster_no =
            ((self.first_cluster_hi() as u32) << 16) | (self.first_cluster_lo() as u32);
        Inode(cluster_no)
    }

    fn get_entry(&self) -> DirEntry {
        let mut result = DirEntry {
            name: ShortFileName {
                contents: [0u8; 11],
            },
            mtime: Timestamp::from_fat(self.write_date(), self.write_time()),
            ctime: Timestamp::from_fat(self.create_date(), self.create_time()),
            attributes: Attributes::create_from_fat(self.raw_attr()),
            inode: self.first_cluster(),
            size: self.file_size(),
        };
        result.name.contents.copy_from_slice(&self.data[0..11]);
        result
    }
}

impl Fat16Volume {
    /// Get an entry from the FAT
    fn get_fat<D, T>(
        &self,
        controller: &mut Controller<D, T>,
        inode: Inode,
    ) -> Result<Inode, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        controller
            .block_device
            .read(&mut blocks, self.lba_start)
            .map_err(|e| Error::DeviceError(e))?;
        let bpb = Bpb::new(&blocks[0]).map_err(|e| Error::FormatError(e))?;
        let fat_size = bpb.fat_size();
        // FAT16 => 2 bytes per entry
        let fat_offset = inode.0 * 2;
        // This is the sector in the FAT that contains this Inode.
        let this_fat_sector_num =
            bpb.reserved_sector_count() as u32 + (fat_offset / Block::LEN as u32);
        let this_fat_ent_offset = fat_offset as usize % Block::LEN;
        controller
            .block_device
            .read(
                &mut blocks,
                BlockIdx(self.lba_start.0 + this_fat_sector_num),
            ).map_err(|e| Error::DeviceError(e))?;
        let entry =
            LittleEndian::read_u16(&blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 1]);
        Ok(Inode(entry as u32))
    }

    /// Write a new entry in the FAT
    fn update_fat<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        inode: Inode,
        new_value: Inode,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        controller
            .block_device
            .read(&mut blocks, self.lba_start)
            .map_err(|e| Error::DeviceError(e))?;
        let (this_fat_sector_num, this_fat_ent_offset) = {
            let bpb = Bpb::new(&blocks[0]).map_err(|e| Error::FormatError(e))?;
            let fat_size = bpb.fat_size();
            // FAT16 => 2 bytes per entry
            let fat_offset = inode.0 * 2;
            // This is the sector in the FAT that contains this Inode.
            let this_fat_sector_num =
                bpb.reserved_sector_count() as u32 + (fat_offset / Block::LEN as u32);
            let this_fat_ent_offset = fat_offset as usize % Block::LEN;
            (BlockIdx(this_fat_sector_num), this_fat_ent_offset)
        };
        controller
            .block_device
            .read(&mut blocks, self.lba_start + this_fat_sector_num)
            .map_err(|e| Error::DeviceError(e))?;
        let entry = match new_value {
            Inode::INVALID => 0xFFFF,
            Inode::BAD => 0xFFF7,
            Inode::EMPTY => 0x0000,
            _ => new_value.0 as u16,
        };
        LittleEndian::write_u16(
            &mut blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 1],
            entry,
        );
        controller
            .block_device
            .write(&blocks, self.lba_start + this_fat_sector_num)
            .map_err(|e| Error::DeviceError(e))?;
        Ok(())
    }

    /// Converts a cluster number (or `Inode`) to a sector number (or
    /// `BlockIdx`). Gives an absolute `BlockIdx` you can pass to the
    /// controller.
    fn inode_to_block<D, T>(
        &self,
        controller: &mut Controller<D, T>,
        inode: Inode,
    ) -> Result<BlockIdx, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        controller
            .block_device
            .read(&mut blocks, self.lba_start)
            .map_err(|e| Error::DeviceError(e))?;
        let bpb = Bpb::new(&blocks[0]).map_err(|e| Error::FormatError(e))?;

        // RootDirSectors = ((BPB_RootEntCnt * 32) + (BPB_BytsPerSec – 1)) / BPB_BytsPerSec;
        let root_dir_sectors =
            ((bpb.root_entries_count() as u32 * 32) + (Block::LEN as u32 - 1)) / Block::LEN as u32;
        // FirstDataSector = BPB_ResvdSecCnt + (BPB_NumFATs * FATSz) + RootDirSectors;
        let first_data_sector = bpb.reserved_sector_count() as u32
            + (bpb.num_fats() as u32 * bpb.fat_size() as u32)
            + root_dir_sectors;
        // FirstSectorofCluster = ((N – 2) * BPB_SecPerClus) + FirstDataSector;
        let first_sector_of_cluster =
            ((inode.0 - 2) * bpb.sectors_per_cluster() as u32) + first_data_sector;
        Ok(BlockIdx(first_sector_of_cluster + self.lba_start.0))
    }

    pub(crate) fn get_root_directory<D, T>(
        &self,
        controller: &mut Controller<D, T>,
    ) -> Result<Directory, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        Ok(Directory {
            inode: Inode::ROOT_DIR,
        })
    }

    /// Get an entry from the given directory
    pub(crate) fn find_dir_entry<D, T>(
        &self,
        controller: &mut Controller<D, T>,
        dir: &Directory,
        name: &str,
    ) -> Result<DirEntry, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let match_name = ShortFileName::new(name).map_err(|e| Error::FilenameError(e))?;
        match dir.inode {
            Inode::ROOT_DIR => {
                // Root
                let mut blocks = [Block::new()];
                controller
                    .block_device
                    .read(&mut blocks, self.lba_start)
                    .map_err(|e| Error::DeviceError(e))?;
                let (first_root_dir_sector_num, root_entries) = {
                    let bpb = Bpb::new(&blocks[0]).map_err(|e| Error::FormatError(e))?;
                    // FirstRootDirSecNum = BPB_ResvdSecCnt + (BPB_NumFATs * BPB_FATSz16);
                    let first_root_dir_sector_num = bpb.reserved_sector_count() as u32
                        + (bpb.num_fats() as u32 * bpb.fat_size() as u32);
                    let root_entries = bpb.root_entries_count();
                    (first_root_dir_sector_num as u32, root_entries as u32)
                };
                for sector in first_root_dir_sector_num..first_root_dir_sector_num + root_entries {
                    controller
                        .block_device
                        .read(&mut blocks, self.lba_start + BlockIdx(sector))
                        .map_err(|e| Error::DeviceError(e))?;
                    for entry in 0..Block::LEN / OnDiskDirEntry::LEN {
                        let start = entry * OnDiskDirEntry::LEN;
                        let end = (entry + 1) * OnDiskDirEntry::LEN;
                        let dir_entry = OnDiskDirEntry::new(&blocks[0][start..end]);
                        if dir_entry.is_end() {
                            // Can quit early
                            return Err(Error::FileNotFound);
                        } else if dir_entry.matches(&match_name) {
                            // Found it
                            return Ok(dir_entry.get_entry());
                        }
                    }
                }
                Err(Error::FileNotFound)
            }
            _ => {
                unimplemented!();
            }
        }
    }

    /// Calls callback `func` with every valid entry in the given directory.
    /// Useful for performing directory listings.
    pub(crate) fn iterate_dir<D, T, F>(
        &self,
        controller: &Controller<D, T>,
        dir: &Directory,
        func: F,
    ) -> Result<(), Error<D::Error>>
    where
        F: Fn(&DirEntry),
        D: BlockDevice,
        T: TimeSource,
    {
        match dir.inode {
            Inode::ROOT_DIR => {
                // Root
                let mut blocks = [Block::new()];
                controller
                    .block_device
                    .read(&mut blocks, self.lba_start)
                    .map_err(|e| Error::DeviceError(e))?;
                let (first_root_dir_sector_num, root_entries) = {
                    let bpb = Bpb::new(&blocks[0]).map_err(|e| Error::FormatError(e))?;
                    // FirstRootDirSecNum = BPB_ResvdSecCnt + (BPB_NumFATs * BPB_FATSz16);
                    let first_root_dir_sector_num = bpb.reserved_sector_count() as u32
                        + (bpb.num_fats() as u32 * bpb.fat_size() as u32);
                    let root_entries = bpb.root_entries_count();
                    (first_root_dir_sector_num as u32, root_entries as u32)
                };
                for sector in first_root_dir_sector_num..first_root_dir_sector_num + root_entries {
                    controller
                        .block_device
                        .read(&mut blocks, self.lba_start + BlockIdx(sector))
                        .map_err(|e| Error::DeviceError(e))?;
                    for entry in 0..Block::LEN / OnDiskDirEntry::LEN {
                        let start = entry * OnDiskDirEntry::LEN;
                        let end = (entry + 1) * OnDiskDirEntry::LEN;
                        let dir_entry = OnDiskDirEntry::new(&blocks[0][start..end]);
                        if dir_entry.is_end() {
                            // Can quit early
                            return Ok(());
                        } else if dir_entry.is_valid() && !dir_entry.is_lfn() {
                            let entry = dir_entry.get_entry();
                            func(&entry);
                        }
                    }
                }
                Ok(())
            }
            _ => {
                unimplemented!();
            }
        }
    }
}

impl Fat32Volume {
    /// Get an entry from the FAT
    fn get_fat<D, T>(
        &self,
        controller: &mut Controller<D, T>,
        inode: Inode,
    ) -> Result<Inode, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        controller
            .block_device
            .read(&mut blocks, self.lba_start)
            .map_err(|e| Error::DeviceError(e))?;
        let bpb = Bpb::new(&blocks[0]).map_err(|e| Error::FormatError(e))?;
        let fat_size = bpb.fat_size();
        // FAT32 => 4 bytes per entry
        let fat_offset = inode.0 * 4;
        // This is the sector in the FAT that contains this Inode.
        let this_fat_sector_num =
            bpb.reserved_sector_count() as u32 + (fat_offset / Block::LEN as u32);
        let this_fat_ent_offset = fat_offset as usize % Block::LEN;
        controller
            .block_device
            .read(
                &mut blocks,
                BlockIdx(self.lba_start.0 + this_fat_sector_num),
            ).map_err(|e| Error::DeviceError(e))?;
        let mut entry =
            LittleEndian::read_u32(&blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 3]);
        entry &= 0x0FFFFFFF;
        Ok(Inode(entry))
    }

    /// Write a new entry in the FAT
    fn update_fat<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        inode: Inode,
        new_value: Inode,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        controller
            .block_device
            .read(&mut blocks, self.lba_start)
            .map_err(|e| Error::DeviceError(e))?;
        let (this_fat_sector_num, this_fat_ent_offset) = {
            let bpb = Bpb::new(&blocks[0]).map_err(|e| Error::FormatError(e))?;
            let fat_size = bpb.fat_size();
            // FAT32 => 2 bytes per entry
            let fat_offset = inode.0 * 4;
            // This is the sector in the FAT that contains this Inode.
            let this_fat_sector_num =
                bpb.reserved_sector_count() as u32 + (fat_offset / Block::LEN as u32);
            let this_fat_ent_offset = fat_offset as usize % Block::LEN;
            (BlockIdx(this_fat_sector_num), this_fat_ent_offset)
        };
        controller
            .block_device
            .read(&mut blocks, self.lba_start + this_fat_sector_num)
            .map_err(|e| Error::DeviceError(e))?;
        let entry = match new_value {
            Inode::INVALID => 0x0FFFFFFF,
            Inode::BAD => 0x0FFFFFF7,
            Inode::EMPTY => 0x0000,
            _ => new_value.0,
        };
        let existing =
            LittleEndian::read_u32(&mut blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 3]);
        let new = (existing & 0xF000_0000) | (entry & 0x0FFF_FFFF);
        LittleEndian::write_u32(
            &mut blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 3],
            new,
        );
        controller
            .block_device
            .write(&blocks, self.lba_start + this_fat_sector_num)
            .map_err(|e| Error::DeviceError(e))?;
        Ok(())
    }

    /// Converts a cluster number (or `Inode`) to a sector number (or
    /// `BlockIdx`). Gives an absolute `BlockIdx` you can pass to the
    /// controller.
    fn inode_to_block<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        inode: Inode,
    ) -> Result<BlockIdx, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        controller
            .block_device
            .read(&mut blocks, self.lba_start)
            .map_err(|e| Error::DeviceError(e))?;
        let bpb = Bpb::new(&blocks[0]).map_err(|e| Error::FormatError(e))?;

        // RootDirSectors = ((BPB_RootEntCnt * 32) + (BPB_BytsPerSec – 1)) / BPB_BytsPerSec;
        let root_dir_sectors =
            ((bpb.root_entries_count() as u32 * 32) + (Block::LEN as u32 - 1)) / Block::LEN as u32;
        // FirstDataSector = BPB_ResvdSecCnt + (BPB_NumFATs * FATSz) + RootDirSectors;
        let first_data_sector = bpb.reserved_sector_count() as u32
            + (bpb.num_fats() as u32 * bpb.fat_size() as u32)
            + root_dir_sectors;
        // FirstSectorofCluster = ((N – 2) * BPB_SecPerClus) + FirstDataSector;
        let first_sector_of_cluster =
            ((inode.0 - 2) * bpb.sectors_per_cluster() as u32) + first_data_sector;
        Ok(BlockIdx(first_sector_of_cluster + self.lba_start.0))
    }

    fn get_root_directory<D, T>(
        &self,
        controller: &mut Controller<D, T>,
    ) -> Result<Directory, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        unimplemented!();
    }

    /// Get an entry from the given directory
    fn find_dir_entry<D, T>(
        &self,
        controller: &mut Controller<D, T>,
        dir: &Directory,
        name: &str,
    ) -> Result<DirEntry, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        unimplemented!();
    }
}

/// Load the boot parameter block from the start of the given partition and
/// determine if the partition contains a valid FAT16 or FAT32 file system.
pub fn parse_volume<D, T>(
    controller: &mut Controller<D, T>,
    lba_start: BlockIdx,
    num_blocks: BlockIdx,
) -> Result<VolumeType, Error<D::Error>>
where
    D: BlockDevice,
    T: TimeSource,
    D::Error: core::fmt::Debug,
{
    let mut blocks = [Block::new()];
    controller
        .block_device
        .read(&mut blocks, lba_start)
        .map_err(|e| Error::DeviceError(e))?;
    let block = &blocks[0];
    let bpb = Bpb::new(&block).map_err(|e| Error::FormatError(e))?;
    if bpb.fat16 {
        let mut volume = Fat16Volume {
            lba_start,
            num_blocks,
            name: [0u8; 11],
        };
        volume.name[..].copy_from_slice(bpb.volume_label());
        Ok(VolumeType::Fat16(volume))
    } else {
        let mut volume = Fat32Volume {
            lba_start,
            num_blocks,
            name: [0u8; 11],
        };
        volume.name[..].copy_from_slice(bpb.volume_label());
        Ok(VolumeType::Fat32(volume))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_bpb() {
        // Taken from a Raspberry Pi bootable SD-Card
        const BPB_EXAMPLE: [u8; 512] = [
            0xeb, 0x3c, 0x90, 0x6d, 0x6b, 0x66, 0x73, 0x2e, 0x66, 0x61, 0x74, 0x00, 0x02, 0x10,
            0x01, 0x00, 0x02, 0x00, 0x02, 0x00, 0x00, 0xf8, 0x20, 0x00, 0x3f, 0x00, 0xff, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0xe0, 0x01, 0x00, 0x80, 0x01, 0x29, 0xbb, 0xb0, 0x71,
            0x77, 0x62, 0x6f, 0x6f, 0x74, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x46, 0x41,
            0x54, 0x31, 0x36, 0x20, 0x20, 0x20, 0x0e, 0x1f, 0xbe, 0x5b, 0x7c, 0xac, 0x22, 0xc0,
            0x74, 0x0b, 0x56, 0xb4, 0x0e, 0xbb, 0x07, 0x00, 0xcd, 0x10, 0x5e, 0xeb, 0xf0, 0x32,
            0xe4, 0xcd, 0x16, 0xcd, 0x19, 0xeb, 0xfe, 0x54, 0x68, 0x69, 0x73, 0x20, 0x69, 0x73,
            0x20, 0x6e, 0x6f, 0x74, 0x20, 0x61, 0x20, 0x62, 0x6f, 0x6f, 0x74, 0x61, 0x62, 0x6c,
            0x65, 0x20, 0x64, 0x69, 0x73, 0x6b, 0x2e, 0x20, 0x20, 0x50, 0x6c, 0x65, 0x61, 0x73,
            0x65, 0x20, 0x69, 0x6e, 0x73, 0x65, 0x72, 0x74, 0x20, 0x61, 0x20, 0x62, 0x6f, 0x6f,
            0x74, 0x61, 0x62, 0x6c, 0x65, 0x20, 0x66, 0x6c, 0x6f, 0x70, 0x70, 0x79, 0x20, 0x61,
            0x6e, 0x64, 0x0d, 0x0a, 0x70, 0x72, 0x65, 0x73, 0x73, 0x20, 0x61, 0x6e, 0x79, 0x20,
            0x6b, 0x65, 0x79, 0x20, 0x74, 0x6f, 0x20, 0x74, 0x72, 0x79, 0x20, 0x61, 0x67, 0x61,
            0x69, 0x6e, 0x20, 0x2e, 0x2e, 0x2e, 0x20, 0x0d, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x55, 0xaa,
        ];
        let bpb = Bpb::new(&BPB_EXAMPLE).unwrap();
        assert_eq!(bpb.footer(), Bpb::FOOTER_VALUE);
        assert_eq!(bpb.oem_name(), b"mkfs.fat");
        assert_eq!(bpb.bytes_per_sector(), 512);
        assert_eq!(bpb.sectors_per_cluster(), 16);
        assert_eq!(bpb.reserved_sector_count(), 1);
        assert_eq!(bpb.num_fats(), 2);
        assert_eq!(bpb.root_entries_count(), 512);
        assert_eq!(bpb.total_sectors16(), 0);
        assert!((bpb.media() & 0xF0) == 0xF0);
        assert_eq!(bpb.fat_size16(), 32);
        assert_eq!(bpb.sectors_per_track(), 63);
        assert_eq!(bpb.num_heads(), 255);
        assert_eq!(bpb.hidden_sectors(), 0);
        assert_eq!(bpb.total_sectors32(), 122880);
        assert_eq!(bpb.footer(), 0xAA55);
        assert_eq!(bpb.drive_number(), 0x80);
        assert_eq!(bpb.boot_signature(), 0x29);
        assert_eq!(bpb.volume_id(), 0x7771B0BB);
        assert_eq!(bpb.volume_label(), b"boot       ");
        assert_eq!(bpb.fs_type(), b"FAT16   ");
        assert_eq!(bpb.fat_size(), 32);
        assert_eq!(bpb.total_sectors(), 122880);
        assert_eq!(bpb.fat16, true);
    }
}
