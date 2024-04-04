//! FAT-specific volume support.

use crate::{
    debug,
    fat::{
        Bpb, Fat16Info, Fat32Info, FatSpecificInfo, FatType, InfoSector, OnDiskDirEntry,
        RESERVED_ENTRIES,
    },
    trace, warn, Attributes, Block, BlockCount, BlockDevice, BlockIdx, ClusterId, DirEntry,
    DirectoryInfo, Error, ShortFileName, TimeSource, VolumeType,
};
use byteorder::{ByteOrder, LittleEndian};
use core::convert::TryFrom;

use super::BlockCache;

/// The name given to a particular FAT formatted volume.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Clone, PartialEq, Eq)]
pub struct VolumeName {
    data: [u8; 11],
}

impl VolumeName {
    /// Create a new VolumeName
    pub fn new(data: [u8; 11]) -> VolumeName {
        VolumeName { data }
    }
}

impl core::fmt::Debug for VolumeName {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        match core::str::from_utf8(&self.data) {
            Ok(s) => write!(fmt, "{:?}", s),
            Err(_e) => write!(fmt, "{:?}", &self.data),
        }
    }
}

/// Identifies a FAT16 or FAT32 Volume on the disk.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, PartialEq, Eq)]
pub struct FatVolume {
    /// The block number of the start of the partition. All other BlockIdx values are relative to this.
    pub(crate) lba_start: BlockIdx,
    /// The number of blocks in this volume
    pub(crate) num_blocks: BlockCount,
    /// The name of this volume
    pub(crate) name: VolumeName,
    /// Number of 512 byte blocks (or Blocks) in a cluster
    pub(crate) blocks_per_cluster: u8,
    /// The block the data starts in. Relative to start of partition (so add
    /// `self.lba_offset` before passing to volume manager)
    pub(crate) first_data_block: BlockCount,
    /// The block the FAT starts in. Relative to start of partition (so add
    /// `self.lba_offset` before passing to volume manager)
    pub(crate) fat_start: BlockCount,
    /// Expected number of free clusters
    pub(crate) free_clusters_count: Option<u32>,
    /// Number of the next expected free cluster
    pub(crate) next_free_cluster: Option<ClusterId>,
    /// Total number of clusters
    pub(crate) cluster_count: u32,
    /// Type of FAT
    pub(crate) fat_specific_info: FatSpecificInfo,
}

impl FatVolume {
    /// Write a new entry in the FAT
    pub fn update_info_sector<D>(&mut self, block_device: &D) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
    {
        match &self.fat_specific_info {
            FatSpecificInfo::Fat16(_) => {
                // FAT16 volumes don't have an info sector
            }
            FatSpecificInfo::Fat32(fat32_info) => {
                if self.free_clusters_count.is_none() && self.next_free_cluster.is_none() {
                    return Ok(());
                }
                let mut blocks = [Block::new()];
                block_device
                    .read(&mut blocks, fat32_info.info_location, "read_info_sector")
                    .map_err(Error::DeviceError)?;
                let block = &mut blocks[0];
                if let Some(count) = self.free_clusters_count {
                    block[488..492].copy_from_slice(&count.to_le_bytes());
                }
                if let Some(next_free_cluster) = self.next_free_cluster {
                    block[492..496].copy_from_slice(&next_free_cluster.0.to_le_bytes());
                }
                block_device
                    .write(&blocks, fat32_info.info_location)
                    .map_err(Error::DeviceError)?;
            }
        }
        Ok(())
    }

    /// Get the type of FAT this volume is
    pub(crate) fn get_fat_type(&self) -> FatType {
        match &self.fat_specific_info {
            FatSpecificInfo::Fat16(_) => FatType::Fat16,
            FatSpecificInfo::Fat32(_) => FatType::Fat32,
        }
    }

    /// Write a new entry in the FAT
    fn update_fat<D>(
        &mut self,
        block_device: &D,
        cluster: ClusterId,
        new_value: ClusterId,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
    {
        let mut blocks = [Block::new()];
        let this_fat_block_num;
        match &self.fat_specific_info {
            FatSpecificInfo::Fat16(_fat16_info) => {
                let fat_offset = cluster.0 * 2;
                this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
                let this_fat_ent_offset = (fat_offset % Block::LEN_U32) as usize;
                block_device
                    .read(&mut blocks, this_fat_block_num, "read_fat")
                    .map_err(Error::DeviceError)?;
                // See <https://en.wikipedia.org/wiki/Design_of_the_FAT_file_system>
                let entry = match new_value {
                    ClusterId::INVALID => 0xFFF6,
                    ClusterId::BAD => 0xFFF7,
                    ClusterId::EMPTY => 0x0000,
                    ClusterId::END_OF_FILE => 0xFFFF,
                    _ => new_value.0 as u16,
                };
                LittleEndian::write_u16(
                    &mut blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 1],
                    entry,
                );
            }
            FatSpecificInfo::Fat32(_fat32_info) => {
                // FAT32 => 4 bytes per entry
                let fat_offset = cluster.0 * 4;
                this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
                let this_fat_ent_offset = (fat_offset % Block::LEN_U32) as usize;
                block_device
                    .read(&mut blocks, this_fat_block_num, "read_fat")
                    .map_err(Error::DeviceError)?;
                let entry = match new_value {
                    ClusterId::INVALID => 0x0FFF_FFF6,
                    ClusterId::BAD => 0x0FFF_FFF7,
                    ClusterId::EMPTY => 0x0000_0000,
                    _ => new_value.0,
                };
                let existing = LittleEndian::read_u32(
                    &blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 3],
                );
                let new = (existing & 0xF000_0000) | (entry & 0x0FFF_FFFF);
                LittleEndian::write_u32(
                    &mut blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 3],
                    new,
                );
            }
        }
        block_device
            .write(&blocks, this_fat_block_num)
            .map_err(Error::DeviceError)?;
        Ok(())
    }

    /// Look in the FAT to see which cluster comes next.
    pub(crate) fn next_cluster<D>(
        &self,
        block_device: &D,
        cluster: ClusterId,
        fat_block_cache: &mut BlockCache,
    ) -> Result<ClusterId, Error<D::Error>>
    where
        D: BlockDevice,
    {
        if cluster.0 > (u32::MAX / 4) {
            panic!("next_cluster called on invalid cluster {:x?}", cluster);
        }
        match &self.fat_specific_info {
            FatSpecificInfo::Fat16(_fat16_info) => {
                let fat_offset = cluster.0 * 2;
                let this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
                let this_fat_ent_offset = (fat_offset % Block::LEN_U32) as usize;
                let block =
                    fat_block_cache.read(block_device, this_fat_block_num, "next_cluster")?;
                let fat_entry =
                    LittleEndian::read_u16(&block[this_fat_ent_offset..=this_fat_ent_offset + 1]);
                match fat_entry {
                    0xFFF7 => {
                        // Bad cluster
                        Err(Error::BadCluster)
                    }
                    0xFFF8..=0xFFFF => {
                        // There is no next cluster
                        Err(Error::EndOfFile)
                    }
                    f => {
                        // Seems legit
                        Ok(ClusterId(u32::from(f)))
                    }
                }
            }
            FatSpecificInfo::Fat32(_fat32_info) => {
                let fat_offset = cluster.0 * 4;
                let this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
                let this_fat_ent_offset = (fat_offset % Block::LEN_U32) as usize;
                let block =
                    fat_block_cache.read(block_device, this_fat_block_num, "next_cluster")?;
                let fat_entry =
                    LittleEndian::read_u32(&block[this_fat_ent_offset..=this_fat_ent_offset + 3])
                        & 0x0FFF_FFFF;
                match fat_entry {
                    0x0000_0000 => {
                        // Jumped to free space
                        Err(Error::UnterminatedFatChain)
                    }
                    0x0FFF_FFF7 => {
                        // Bad cluster
                        Err(Error::BadCluster)
                    }
                    0x0000_0001 | 0x0FFF_FFF8..=0x0FFF_FFFF => {
                        // There is no next cluster
                        Err(Error::EndOfFile)
                    }
                    f => {
                        // Seems legit
                        Ok(ClusterId(f))
                    }
                }
            }
        }
    }

    /// Number of bytes in a cluster.
    pub(crate) fn bytes_per_cluster(&self) -> u32 {
        u32::from(self.blocks_per_cluster) * Block::LEN_U32
    }

    /// Converts a cluster number (or `Cluster`) to a block number (or
    /// `BlockIdx`). Gives an absolute `BlockIdx` you can pass to the
    /// volume manager.
    pub(crate) fn cluster_to_block(&self, cluster: ClusterId) -> BlockIdx {
        match &self.fat_specific_info {
            FatSpecificInfo::Fat16(fat16_info) => {
                let block_num = match cluster {
                    ClusterId::ROOT_DIR => fat16_info.first_root_dir_block,
                    ClusterId(c) => {
                        // FirstSectorofCluster = ((N – 2) * BPB_SecPerClus) + FirstDataSector;
                        let first_block_of_cluster =
                            BlockCount((c - 2) * u32::from(self.blocks_per_cluster));
                        self.first_data_block + first_block_of_cluster
                    }
                };
                self.lba_start + block_num
            }
            FatSpecificInfo::Fat32(fat32_info) => {
                let cluster_num = match cluster {
                    ClusterId::ROOT_DIR => fat32_info.first_root_dir_cluster.0,
                    c => c.0,
                };
                // FirstSectorofCluster = ((N – 2) * BPB_SecPerClus) + FirstDataSector;
                let first_block_of_cluster =
                    BlockCount((cluster_num - 2) * u32::from(self.blocks_per_cluster));
                self.lba_start + self.first_data_block + first_block_of_cluster
            }
        }
    }

    /// Finds a empty entry space and writes the new entry to it, allocates a new cluster if it's
    /// needed
    pub(crate) fn write_new_directory_entry<D, T>(
        &mut self,
        block_device: &D,
        time_source: &T,
        dir_cluster: ClusterId,
        name: ShortFileName,
        attributes: Attributes,
    ) -> Result<DirEntry, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        match &self.fat_specific_info {
            FatSpecificInfo::Fat16(fat16_info) => {
                // Root directories on FAT16 have a fixed size, because they use
                // a specially reserved space on disk (see
                // `first_root_dir_block`). Other directories can have any size
                // as they are made of regular clusters.
                let mut current_cluster = Some(dir_cluster);
                let mut first_dir_block_num = match dir_cluster {
                    ClusterId::ROOT_DIR => self.lba_start + fat16_info.first_root_dir_block,
                    _ => self.cluster_to_block(dir_cluster),
                };
                let dir_size = match dir_cluster {
                    ClusterId::ROOT_DIR => {
                        let len_bytes =
                            u32::from(fat16_info.root_entries_count) * OnDiskDirEntry::LEN_U32;
                        BlockCount::from_bytes(len_bytes)
                    }
                    _ => BlockCount(u32::from(self.blocks_per_cluster)),
                };

                // Walk the directory
                let mut blocks = [Block::new()];
                while let Some(cluster) = current_cluster {
                    for block in first_dir_block_num.range(dir_size) {
                        block_device
                            .read(&mut blocks, block, "read_dir")
                            .map_err(Error::DeviceError)?;
                        let entries_per_block = Block::LEN / OnDiskDirEntry::LEN;
                        for entry in 0..entries_per_block {
                            let start = entry * OnDiskDirEntry::LEN;
                            let end = (entry + 1) * OnDiskDirEntry::LEN;
                            let dir_entry = OnDiskDirEntry::new(&blocks[0][start..end]);
                            // 0x00 or 0xE5 represents a free entry
                            if !dir_entry.is_valid() {
                                let ctime = time_source.get_timestamp();
                                let entry = DirEntry::new(
                                    name,
                                    attributes,
                                    ClusterId::EMPTY,
                                    ctime,
                                    block,
                                    start as u32,
                                );
                                blocks[0][start..start + 32]
                                    .copy_from_slice(&entry.serialize(FatType::Fat16)[..]);
                                block_device
                                    .write(&blocks, block)
                                    .map_err(Error::DeviceError)?;
                                return Ok(entry);
                            }
                        }
                    }
                    if cluster != ClusterId::ROOT_DIR {
                        let mut block_cache = BlockCache::empty();
                        current_cluster =
                            match self.next_cluster(block_device, cluster, &mut block_cache) {
                                Ok(n) => {
                                    first_dir_block_num = self.cluster_to_block(n);
                                    Some(n)
                                }
                                Err(Error::EndOfFile) => {
                                    let c =
                                        self.alloc_cluster(block_device, Some(cluster), true)?;
                                    first_dir_block_num = self.cluster_to_block(c);
                                    Some(c)
                                }
                                _ => None,
                            };
                    } else {
                        current_cluster = None;
                    }
                }
                Err(Error::NotEnoughSpace)
            }
            FatSpecificInfo::Fat32(fat32_info) => {
                // All directories on FAT32 have a cluster chain but the root
                // dir starts in a specified cluster.
                let mut current_cluster = match dir_cluster {
                    ClusterId::ROOT_DIR => Some(fat32_info.first_root_dir_cluster),
                    _ => Some(dir_cluster),
                };
                let mut first_dir_block_num = self.cluster_to_block(dir_cluster);
                let mut blocks = [Block::new()];

                let dir_size = BlockCount(u32::from(self.blocks_per_cluster));
                // Walk the cluster chain until we run out of clusters
                while let Some(cluster) = current_cluster {
                    // Loop through the blocks in the cluster
                    for block in first_dir_block_num.range(dir_size) {
                        // Read a block of directory entries
                        block_device
                            .read(&mut blocks, block, "read_dir")
                            .map_err(Error::DeviceError)?;
                        // Are any entries in the block we just loaded blank? If so
                        // we can use them.
                        for entry in 0..Block::LEN / OnDiskDirEntry::LEN {
                            let start = entry * OnDiskDirEntry::LEN;
                            let end = (entry + 1) * OnDiskDirEntry::LEN;
                            let dir_entry = OnDiskDirEntry::new(&blocks[0][start..end]);
                            // 0x00 or 0xE5 represents a free entry
                            if !dir_entry.is_valid() {
                                let ctime = time_source.get_timestamp();
                                let entry = DirEntry::new(
                                    name,
                                    attributes,
                                    ClusterId(0),
                                    ctime,
                                    block,
                                    start as u32,
                                );
                                blocks[0][start..start + 32]
                                    .copy_from_slice(&entry.serialize(FatType::Fat32)[..]);
                                block_device
                                    .write(&blocks, block)
                                    .map_err(Error::DeviceError)?;
                                return Ok(entry);
                            }
                        }
                    }
                    // Well none of the blocks in that cluster had any space in
                    // them, let's fetch another one.
                    let mut block_cache = BlockCache::empty();
                    current_cluster =
                        match self.next_cluster(block_device, cluster, &mut block_cache) {
                            Ok(n) => {
                                first_dir_block_num = self.cluster_to_block(n);
                                Some(n)
                            }
                            Err(Error::EndOfFile) => {
                                let c = self.alloc_cluster(block_device, Some(cluster), true)?;
                                first_dir_block_num = self.cluster_to_block(c);
                                Some(c)
                            }
                            _ => None,
                        };
                }
                // We ran out of clusters in the chain, and apparently we weren't
                // able to make the chain longer, so the disk must be full.
                Err(Error::NotEnoughSpace)
            }
        }
    }

    /// Calls callback `func` with every valid entry in the given directory.
    /// Useful for performing directory listings.
    pub(crate) fn iterate_dir<D, F>(
        &self,
        block_device: &D,
        dir: &DirectoryInfo,
        func: F,
    ) -> Result<(), Error<D::Error>>
    where
        F: FnMut(&DirEntry),
        D: BlockDevice,
    {
        match &self.fat_specific_info {
            FatSpecificInfo::Fat16(fat16_info) => {
                self.iterate_fat16(dir, fat16_info, block_device, func)
            }
            FatSpecificInfo::Fat32(fat32_info) => {
                self.iterate_fat32(dir, fat32_info, block_device, func)
            }
        }
    }

    fn iterate_fat16<D, F>(
        &self,
        dir: &DirectoryInfo,
        fat16_info: &Fat16Info,
        block_device: &D,
        mut func: F,
    ) -> Result<(), Error<D::Error>>
    where
        F: FnMut(&DirEntry),
        D: BlockDevice,
    {
        // Root directories on FAT16 have a fixed size, because they use
        // a specially reserved space on disk (see
        // `first_root_dir_block`). Other directories can have any size
        // as they are made of regular clusters.
        let mut current_cluster = Some(dir.cluster);
        let mut first_dir_block_num = match dir.cluster {
            ClusterId::ROOT_DIR => self.lba_start + fat16_info.first_root_dir_block,
            _ => self.cluster_to_block(dir.cluster),
        };
        let dir_size = match dir.cluster {
            ClusterId::ROOT_DIR => {
                let len_bytes = u32::from(fat16_info.root_entries_count) * OnDiskDirEntry::LEN_U32;
                BlockCount::from_bytes(len_bytes)
            }
            _ => BlockCount(u32::from(self.blocks_per_cluster)),
        };

        let mut block_cache = BlockCache::empty();
        while let Some(cluster) = current_cluster {
            for block_idx in first_dir_block_num.range(dir_size) {
                let block = block_cache.read(block_device, block_idx, "read_dir")?;
                for entry in 0..Block::LEN / OnDiskDirEntry::LEN {
                    let start = entry * OnDiskDirEntry::LEN;
                    let end = (entry + 1) * OnDiskDirEntry::LEN;
                    let dir_entry = OnDiskDirEntry::new(&block[start..end]);
                    if dir_entry.is_end() {
                        // Can quit early
                        return Ok(());
                    } else if dir_entry.is_valid() && !dir_entry.is_lfn() {
                        // Safe, since Block::LEN always fits on a u32
                        let start = u32::try_from(start).unwrap();
                        let entry = dir_entry.get_entry(FatType::Fat16, block_idx, start);
                        func(&entry);
                    }
                }
            }
            if cluster != ClusterId::ROOT_DIR {
                current_cluster = match self.next_cluster(block_device, cluster, &mut block_cache) {
                    Ok(n) => {
                        first_dir_block_num = self.cluster_to_block(n);
                        Some(n)
                    }
                    _ => None,
                };
            } else {
                current_cluster = None;
            }
        }
        Ok(())
    }

    fn iterate_fat32<D, F>(
        &self,
        dir: &DirectoryInfo,
        fat32_info: &Fat32Info,
        block_device: &D,
        mut func: F,
    ) -> Result<(), Error<D::Error>>
    where
        F: FnMut(&DirEntry),
        D: BlockDevice,
    {
        // All directories on FAT32 have a cluster chain but the root
        // dir starts in a specified cluster.
        let mut current_cluster = match dir.cluster {
            ClusterId::ROOT_DIR => Some(fat32_info.first_root_dir_cluster),
            _ => Some(dir.cluster),
        };
        let mut blocks = [Block::new()];
        let mut block_cache = BlockCache::empty();
        while let Some(cluster) = current_cluster {
            let block_idx = self.cluster_to_block(cluster);
            for block in block_idx.range(BlockCount(u32::from(self.blocks_per_cluster))) {
                block_device
                    .read(&mut blocks, block, "read_dir")
                    .map_err(Error::DeviceError)?;
                for entry in 0..Block::LEN / OnDiskDirEntry::LEN {
                    let start = entry * OnDiskDirEntry::LEN;
                    let end = (entry + 1) * OnDiskDirEntry::LEN;
                    let dir_entry = OnDiskDirEntry::new(&blocks[0][start..end]);
                    if dir_entry.is_end() {
                        // Can quit early
                        return Ok(());
                    } else if dir_entry.is_valid() && !dir_entry.is_lfn() {
                        // Safe, since Block::LEN always fits on a u32
                        let start = u32::try_from(start).unwrap();
                        let entry = dir_entry.get_entry(FatType::Fat32, block, start);
                        func(&entry);
                    }
                }
            }
            current_cluster = match self.next_cluster(block_device, cluster, &mut block_cache) {
                Ok(n) => Some(n),
                _ => None,
            };
        }
        Ok(())
    }

    /// Get an entry from the given directory
    pub(crate) fn find_directory_entry<D>(
        &self,
        block_device: &D,
        dir: &DirectoryInfo,
        match_name: &ShortFileName,
    ) -> Result<DirEntry, Error<D::Error>>
    where
        D: BlockDevice,
    {
        match &self.fat_specific_info {
            FatSpecificInfo::Fat16(fat16_info) => {
                // Root directories on FAT16 have a fixed size, because they use
                // a specially reserved space on disk (see
                // `first_root_dir_block`). Other directories can have any size
                // as they are made of regular clusters.
                let mut current_cluster = Some(dir.cluster);
                let mut first_dir_block_num = match dir.cluster {
                    ClusterId::ROOT_DIR => self.lba_start + fat16_info.first_root_dir_block,
                    _ => self.cluster_to_block(dir.cluster),
                };
                let dir_size = match dir.cluster {
                    ClusterId::ROOT_DIR => {
                        let len_bytes =
                            u32::from(fat16_info.root_entries_count) * OnDiskDirEntry::LEN_U32;
                        BlockCount::from_bytes(len_bytes)
                    }
                    _ => BlockCount(u32::from(self.blocks_per_cluster)),
                };

                let mut block_cache = BlockCache::empty();
                while let Some(cluster) = current_cluster {
                    for block in first_dir_block_num.range(dir_size) {
                        match self.find_entry_in_block(
                            block_device,
                            FatType::Fat16,
                            match_name,
                            block,
                        ) {
                            Err(Error::NotFound) => continue,
                            x => return x,
                        }
                    }
                    if cluster != ClusterId::ROOT_DIR {
                        current_cluster =
                            match self.next_cluster(block_device, cluster, &mut block_cache) {
                                Ok(n) => {
                                    first_dir_block_num = self.cluster_to_block(n);
                                    Some(n)
                                }
                                _ => None,
                            };
                    } else {
                        current_cluster = None;
                    }
                }
                Err(Error::NotFound)
            }
            FatSpecificInfo::Fat32(fat32_info) => {
                let mut current_cluster = match dir.cluster {
                    ClusterId::ROOT_DIR => Some(fat32_info.first_root_dir_cluster),
                    _ => Some(dir.cluster),
                };
                let mut block_cache = BlockCache::empty();
                while let Some(cluster) = current_cluster {
                    let block_idx = self.cluster_to_block(cluster);
                    for block in block_idx.range(BlockCount(u32::from(self.blocks_per_cluster))) {
                        match self.find_entry_in_block(
                            block_device,
                            FatType::Fat32,
                            match_name,
                            block,
                        ) {
                            Err(Error::NotFound) => continue,
                            x => return x,
                        }
                    }
                    current_cluster =
                        match self.next_cluster(block_device, cluster, &mut block_cache) {
                            Ok(n) => Some(n),
                            _ => None,
                        }
                }
                Err(Error::NotFound)
            }
        }
    }

    /// Finds an entry in a given block of directory entries.
    fn find_entry_in_block<D>(
        &self,
        block_device: &D,
        fat_type: FatType,
        match_name: &ShortFileName,
        block: BlockIdx,
    ) -> Result<DirEntry, Error<D::Error>>
    where
        D: BlockDevice,
    {
        let mut blocks = [Block::new()];
        block_device
            .read(&mut blocks, block, "read_dir")
            .map_err(Error::DeviceError)?;
        for entry in 0..Block::LEN / OnDiskDirEntry::LEN {
            let start = entry * OnDiskDirEntry::LEN;
            let end = (entry + 1) * OnDiskDirEntry::LEN;
            let dir_entry = OnDiskDirEntry::new(&blocks[0][start..end]);
            if dir_entry.is_end() {
                // Can quit early
                break;
            } else if dir_entry.matches(match_name) {
                // Found it
                // Safe, since Block::LEN always fits on a u32
                let start = u32::try_from(start).unwrap();
                return Ok(dir_entry.get_entry(fat_type, block, start));
            }
        }
        Err(Error::NotFound)
    }

    /// Delete an entry from the given directory
    pub(crate) fn delete_directory_entry<D>(
        &self,
        block_device: &D,
        dir: &DirectoryInfo,
        match_name: &ShortFileName,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
    {
        match &self.fat_specific_info {
            FatSpecificInfo::Fat16(fat16_info) => {
                // Root directories on FAT16 have a fixed size, because they use
                // a specially reserved space on disk (see
                // `first_root_dir_block`). Other directories can have any size
                // as they are made of regular clusters.
                let mut current_cluster = Some(dir.cluster);
                let mut first_dir_block_num = match dir.cluster {
                    ClusterId::ROOT_DIR => self.lba_start + fat16_info.first_root_dir_block,
                    _ => self.cluster_to_block(dir.cluster),
                };
                let dir_size = match dir.cluster {
                    ClusterId::ROOT_DIR => {
                        let len_bytes =
                            u32::from(fat16_info.root_entries_count) * OnDiskDirEntry::LEN_U32;
                        BlockCount::from_bytes(len_bytes)
                    }
                    _ => BlockCount(u32::from(self.blocks_per_cluster)),
                };

                // Walk the directory
                while let Some(cluster) = current_cluster {
                    // Scan the cluster / root dir a block at a time
                    for block in first_dir_block_num.range(dir_size) {
                        match self.delete_entry_in_block(block_device, match_name, block) {
                            Err(Error::NotFound) => {
                                // Carry on
                            }
                            x => {
                                // Either we deleted it OK, or there was some
                                // catastrophic error reading/writing the disk.
                                return x;
                            }
                        }
                    }
                    // if it's not the root dir, find the next cluster so we can keep looking
                    if cluster != ClusterId::ROOT_DIR {
                        let mut block_cache = BlockCache::empty();
                        current_cluster =
                            match self.next_cluster(block_device, cluster, &mut block_cache) {
                                Ok(n) => {
                                    first_dir_block_num = self.cluster_to_block(n);
                                    Some(n)
                                }
                                _ => None,
                            };
                    } else {
                        current_cluster = None;
                    }
                }
                // Ok, give up
            }
            FatSpecificInfo::Fat32(fat32_info) => {
                // Root directories on FAT32 start at a specified cluster, but
                // they can have any length.
                let mut current_cluster = match dir.cluster {
                    ClusterId::ROOT_DIR => Some(fat32_info.first_root_dir_cluster),
                    _ => Some(dir.cluster),
                };
                // Walk the directory
                while let Some(cluster) = current_cluster {
                    // Scan the cluster a block at a time
                    let block_idx = self.cluster_to_block(cluster);
                    for block in block_idx.range(BlockCount(u32::from(self.blocks_per_cluster))) {
                        match self.delete_entry_in_block(block_device, match_name, block) {
                            Err(Error::NotFound) => {
                                // Carry on
                                continue;
                            }
                            x => {
                                // Either we deleted it OK, or there was some
                                // catastrophic error reading/writing the disk.
                                return x;
                            }
                        }
                    }
                    // Find the next cluster
                    let mut block_cache = BlockCache::empty();
                    current_cluster =
                        match self.next_cluster(block_device, cluster, &mut block_cache) {
                            Ok(n) => Some(n),
                            _ => None,
                        }
                }
                // Ok, give up
            }
        }
        // If we get here we never found the right entry in any of the
        // blocks that made up the directory
        Err(Error::NotFound)
    }

    /// Deletes a directory entry from a block of directory entries.
    ///
    /// Entries are marked as deleted by setting the first byte of the file name
    /// to a special value.
    fn delete_entry_in_block<D>(
        &self,
        block_device: &D,
        match_name: &ShortFileName,
        block: BlockIdx,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
    {
        let mut blocks = [Block::new()];
        block_device
            .read(&mut blocks, block, "read_dir")
            .map_err(Error::DeviceError)?;
        for entry in 0..Block::LEN / OnDiskDirEntry::LEN {
            let start = entry * OnDiskDirEntry::LEN;
            let end = (entry + 1) * OnDiskDirEntry::LEN;
            let dir_entry = OnDiskDirEntry::new(&blocks[0][start..end]);
            if dir_entry.is_end() {
                // Can quit early
                break;
            } else if dir_entry.matches(match_name) {
                let mut blocks = blocks;
                blocks[0].contents[start] = 0xE5;
                return block_device
                    .write(&blocks, block)
                    .map_err(Error::DeviceError);
            }
        }
        Err(Error::NotFound)
    }

    /// Finds the next free cluster after the start_cluster and before end_cluster
    pub(crate) fn find_next_free_cluster<D>(
        &self,
        block_device: &D,
        start_cluster: ClusterId,
        end_cluster: ClusterId,
    ) -> Result<ClusterId, Error<D::Error>>
    where
        D: BlockDevice,
    {
        let mut blocks = [Block::new()];
        let mut current_cluster = start_cluster;
        match &self.fat_specific_info {
            FatSpecificInfo::Fat16(_fat16_info) => {
                while current_cluster.0 < end_cluster.0 {
                    trace!(
                        "current_cluster={:?}, end_cluster={:?}",
                        current_cluster,
                        end_cluster
                    );
                    let fat_offset = current_cluster.0 * 2;
                    trace!("fat_offset = {:?}", fat_offset);
                    let this_fat_block_num =
                        self.lba_start + self.fat_start.offset_bytes(fat_offset);
                    trace!("this_fat_block_num = {:?}", this_fat_block_num);
                    let mut this_fat_ent_offset = usize::try_from(fat_offset % Block::LEN_U32)
                        .map_err(|_| Error::ConversionError)?;
                    trace!("Reading block {:?}", this_fat_block_num);
                    block_device
                        .read(&mut blocks, this_fat_block_num, "next_cluster")
                        .map_err(Error::DeviceError)?;

                    while this_fat_ent_offset <= Block::LEN - 2 {
                        let fat_entry = LittleEndian::read_u16(
                            &blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 1],
                        );
                        if fat_entry == 0 {
                            return Ok(current_cluster);
                        }
                        this_fat_ent_offset += 2;
                        current_cluster += 1;
                    }
                }
            }
            FatSpecificInfo::Fat32(_fat32_info) => {
                while current_cluster.0 < end_cluster.0 {
                    trace!(
                        "current_cluster={:?}, end_cluster={:?}",
                        current_cluster,
                        end_cluster
                    );
                    let fat_offset = current_cluster.0 * 4;
                    trace!("fat_offset = {:?}", fat_offset);
                    let this_fat_block_num =
                        self.lba_start + self.fat_start.offset_bytes(fat_offset);
                    trace!("this_fat_block_num = {:?}", this_fat_block_num);
                    let mut this_fat_ent_offset = usize::try_from(fat_offset % Block::LEN_U32)
                        .map_err(|_| Error::ConversionError)?;
                    trace!("Reading block {:?}", this_fat_block_num);
                    block_device
                        .read(&mut blocks, this_fat_block_num, "next_cluster")
                        .map_err(Error::DeviceError)?;

                    while this_fat_ent_offset <= Block::LEN - 4 {
                        let fat_entry = LittleEndian::read_u32(
                            &blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 3],
                        ) & 0x0FFF_FFFF;
                        if fat_entry == 0 {
                            return Ok(current_cluster);
                        }
                        this_fat_ent_offset += 4;
                        current_cluster += 1;
                    }
                }
            }
        }
        warn!("Out of space...");
        Err(Error::NotEnoughSpace)
    }

    /// Tries to allocate a cluster
    pub(crate) fn alloc_cluster<D>(
        &mut self,
        block_device: &D,
        prev_cluster: Option<ClusterId>,
        zero: bool,
    ) -> Result<ClusterId, Error<D::Error>>
    where
        D: BlockDevice,
    {
        debug!("Allocating new cluster, prev_cluster={:?}", prev_cluster);
        let end_cluster = ClusterId(self.cluster_count + RESERVED_ENTRIES);
        let start_cluster = match self.next_free_cluster {
            Some(cluster) if cluster.0 < end_cluster.0 => cluster,
            _ => ClusterId(RESERVED_ENTRIES),
        };
        trace!(
            "Finding next free between {:?}..={:?}",
            start_cluster,
            end_cluster
        );
        let new_cluster =
            match self.find_next_free_cluster(block_device, start_cluster, end_cluster) {
                Ok(cluster) => cluster,
                Err(_) if start_cluster.0 > RESERVED_ENTRIES => {
                    debug!(
                        "Retrying, finding next free between {:?}..={:?}",
                        ClusterId(RESERVED_ENTRIES),
                        end_cluster
                    );
                    self.find_next_free_cluster(
                        block_device,
                        ClusterId(RESERVED_ENTRIES),
                        end_cluster,
                    )?
                }
                Err(e) => return Err(e),
            };
        self.update_fat(block_device, new_cluster, ClusterId::END_OF_FILE)?;
        if let Some(cluster) = prev_cluster {
            trace!(
                "Updating old cluster {:?} to {:?} in FAT",
                cluster,
                new_cluster
            );
            self.update_fat(block_device, cluster, new_cluster)?;
        }
        trace!(
            "Finding next free between {:?}..={:?}",
            new_cluster,
            end_cluster
        );
        self.next_free_cluster =
            match self.find_next_free_cluster(block_device, new_cluster, end_cluster) {
                Ok(cluster) => Some(cluster),
                Err(_) if new_cluster.0 > RESERVED_ENTRIES => {
                    match self.find_next_free_cluster(
                        block_device,
                        ClusterId(RESERVED_ENTRIES),
                        end_cluster,
                    ) {
                        Ok(cluster) => Some(cluster),
                        Err(e) => return Err(e),
                    }
                }
                Err(e) => return Err(e),
            };
        debug!("Next free cluster is {:?}", self.next_free_cluster);
        if let Some(ref mut number_free_cluster) = self.free_clusters_count {
            *number_free_cluster -= 1;
        };
        if zero {
            let blocks = [Block::new()];
            let first_block = self.cluster_to_block(new_cluster);
            let num_blocks = BlockCount(u32::from(self.blocks_per_cluster));
            for block in first_block.range(num_blocks) {
                block_device
                    .write(&blocks, block)
                    .map_err(Error::DeviceError)?;
            }
        }
        debug!("All done, returning {:?}", new_cluster);
        Ok(new_cluster)
    }

    /// Marks the input cluster as an EOF and all the subsequent clusters in the chain as free
    pub(crate) fn truncate_cluster_chain<D>(
        &mut self,
        block_device: &D,
        cluster: ClusterId,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
    {
        if cluster.0 < RESERVED_ENTRIES {
            // file doesn't have any valid cluster allocated, there is nothing to do
            return Ok(());
        }
        let mut next = {
            let mut block_cache = BlockCache::empty();
            match self.next_cluster(block_device, cluster, &mut block_cache) {
                Ok(n) => n,
                Err(Error::EndOfFile) => return Ok(()),
                Err(e) => return Err(e),
            }
        };
        if let Some(ref mut next_free_cluster) = self.next_free_cluster {
            if next_free_cluster.0 > next.0 {
                *next_free_cluster = next;
            }
        } else {
            self.next_free_cluster = Some(next);
        }
        self.update_fat(block_device, cluster, ClusterId::END_OF_FILE)?;
        loop {
            let mut block_cache = BlockCache::empty();
            match self.next_cluster(block_device, next, &mut block_cache) {
                Ok(n) => {
                    self.update_fat(block_device, next, ClusterId::EMPTY)?;
                    next = n;
                }
                Err(Error::EndOfFile) => {
                    self.update_fat(block_device, next, ClusterId::EMPTY)?;
                    break;
                }
                Err(e) => return Err(e),
            }
            if let Some(ref mut number_free_cluster) = self.free_clusters_count {
                *number_free_cluster += 1;
            };
        }
        Ok(())
    }

    /// Writes a Directory Entry to the disk
    pub(crate) fn write_entry_to_disk<D>(
        &self,
        block_device: &D,
        entry: &DirEntry,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
    {
        let fat_type = match self.fat_specific_info {
            FatSpecificInfo::Fat16(_) => FatType::Fat16,
            FatSpecificInfo::Fat32(_) => FatType::Fat32,
        };
        let mut blocks = [Block::new()];
        block_device
            .read(&mut blocks, entry.entry_block, "read")
            .map_err(Error::DeviceError)?;
        let block = &mut blocks[0];

        let start = usize::try_from(entry.entry_offset).map_err(|_| Error::ConversionError)?;
        block[start..start + 32].copy_from_slice(&entry.serialize(fat_type)[..]);

        block_device
            .write(&blocks, entry.entry_block)
            .map_err(Error::DeviceError)?;
        Ok(())
    }
}

/// Load the boot parameter block from the start of the given partition and
/// determine if the partition contains a valid FAT16 or FAT32 file system.
pub fn parse_volume<D>(
    block_device: &D,
    lba_start: BlockIdx,
    num_blocks: BlockCount,
) -> Result<VolumeType, Error<D::Error>>
where
    D: BlockDevice,
    D::Error: core::fmt::Debug,
{
    let mut blocks = [Block::new()];
    block_device
        .read(&mut blocks, lba_start, "read_bpb")
        .map_err(Error::DeviceError)?;
    let block = &blocks[0];
    let bpb = Bpb::create_from_bytes(block).map_err(Error::FormatError)?;
    match bpb.fat_type {
        FatType::Fat16 => {
            if bpb.bytes_per_block() as usize != Block::LEN {
                return Err(Error::BadBlockSize(bpb.bytes_per_block()));
            }
            // FirstDataSector = BPB_ResvdSecCnt + (BPB_NumFATs * FATSz) + RootDirSectors;
            let root_dir_blocks = ((u32::from(bpb.root_entries_count()) * OnDiskDirEntry::LEN_U32)
                + (Block::LEN_U32 - 1))
                / Block::LEN_U32;
            let fat_start = BlockCount(u32::from(bpb.reserved_block_count()));
            let first_root_dir_block =
                fat_start + BlockCount(u32::from(bpb.num_fats()) * bpb.fat_size());
            let first_data_block = first_root_dir_block + BlockCount(root_dir_blocks);
            let mut volume = FatVolume {
                lba_start,
                num_blocks,
                name: VolumeName { data: [0u8; 11] },
                blocks_per_cluster: bpb.blocks_per_cluster(),
                first_data_block: (first_data_block),
                fat_start: BlockCount(u32::from(bpb.reserved_block_count())),
                free_clusters_count: None,
                next_free_cluster: None,
                cluster_count: bpb.total_clusters(),
                fat_specific_info: FatSpecificInfo::Fat16(Fat16Info {
                    root_entries_count: bpb.root_entries_count(),
                    first_root_dir_block,
                }),
            };
            volume.name.data[..].copy_from_slice(bpb.volume_label());
            Ok(VolumeType::Fat(volume))
        }
        FatType::Fat32 => {
            // FirstDataSector = BPB_ResvdSecCnt + (BPB_NumFATs * FATSz);
            let first_data_block = u32::from(bpb.reserved_block_count())
                + (u32::from(bpb.num_fats()) * bpb.fat_size());

            // Safe to unwrap since this is a Fat32 Type
            let info_location = bpb.fs_info_block().unwrap();
            let mut info_blocks = [Block::new()];
            block_device
                .read(
                    &mut info_blocks,
                    lba_start + info_location,
                    "read_info_sector",
                )
                .map_err(Error::DeviceError)?;
            let info_block = &info_blocks[0];
            let info_sector =
                InfoSector::create_from_bytes(info_block).map_err(Error::FormatError)?;

            let mut volume = FatVolume {
                lba_start,
                num_blocks,
                name: VolumeName { data: [0u8; 11] },
                blocks_per_cluster: bpb.blocks_per_cluster(),
                first_data_block: BlockCount(first_data_block),
                fat_start: BlockCount(u32::from(bpb.reserved_block_count())),
                free_clusters_count: info_sector.free_clusters_count(),
                next_free_cluster: info_sector.next_free_cluster(),
                cluster_count: bpb.total_clusters(),
                fat_specific_info: FatSpecificInfo::Fat32(Fat32Info {
                    info_location: lba_start + info_location,
                    first_root_dir_cluster: ClusterId(bpb.first_root_dir_cluster()),
                }),
            };
            volume.name.data[..].copy_from_slice(bpb.volume_label());
            Ok(VolumeType::Fat(volume))
        }
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
