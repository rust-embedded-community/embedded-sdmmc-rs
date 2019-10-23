//! embedded-sdmmc-rs - FAT16/FAT32 file system implementation
//!
//! Implements the File Allocation Table file system. Supports FAT16 and FAT32 volumes.

use crate::blockdevice::BlockCount;
use crate::{
    Attributes, Block, BlockDevice, BlockIdx, Cluster, Controller, DirEntry, Directory, Error,
    ShortFileName, TimeSource, Timestamp, VolumeType,
};
use byteorder::{ByteOrder, LittleEndian};
use core::convert::TryFrom;

pub(crate) const RESERVED_ENTRIES: u32 = 2;

/// Indentifies the supported types of FAT format
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FatType {
    /// Fat16 Format
    Fat16,
    /// Fat32 Format
    Fat32,
}

#[derive(PartialEq, Eq)]
pub(crate) struct VolumeName {
    pub(crate) data: [u8; 11],
}

/// Identifies a FAT16 Volume on the disk.
#[derive(PartialEq, Eq, Debug)]
pub struct Fat16Volume {
    /// The block number of the start of the partition. All other BlockIdx values are relative to this.
    pub(crate) lba_start: BlockIdx,
    /// The number of blocks in this volume
    pub(crate) num_blocks: BlockCount,
    /// The name of this volume
    pub(crate) name: VolumeName,
    /// Number of 512 byte blocks (or Blocks) in a cluster
    pub(crate) blocks_per_cluster: u8,
    /// The block the data starts in. Relative to start of partition (so add `self.lba_offset` before passing to controller)
    pub(crate) first_data_block: BlockCount,
    /// The block the FAT starts in. Relative to start of partition (so add `self.lba_offset` before passing to controller)
    pub(crate) fat_start: BlockCount,
    /// The block the root directory starts in. Relative to start of partition (so add `self.lba_offset` before passing to controller)
    pub(crate) first_root_dir_block: BlockCount,
    /// Number of entries in root directory (it's reserved and not in the FAT)
    pub(crate) root_entries_count: u16,
    /// Expected number of free clusters
    pub(crate) free_clusters_count: Option<u32>,
    /// Number of the next expected free cluster
    pub(crate) next_free_cluster: Option<Cluster>,
    /// Total number of clusters
    pub(crate) cluster_count: u32,
}

/// Identifies a FAT32 Volume on the disk.
#[derive(PartialEq, Eq, Debug)]
pub struct Fat32Volume {
    /// The block number of the start of the partition. All other BlockIdx values are relative to this.
    pub(crate) lba_start: BlockIdx,
    /// The number of blocks in this volume
    pub(crate) num_blocks: BlockCount,
    /// The name of this volume
    pub(crate) name: VolumeName,
    /// Number of 512 byte blocks (or Blocks) in a cluster
    pub(crate) blocks_per_cluster: u8,
    /// Relative to start of partition (so add `self.lba_offset` before passing to controller)
    pub(crate) first_data_block: BlockCount,
    /// The block the FAT starts in. Relative to start of partition (so add `self.lba_offset` before passing to controller)
    pub(crate) fat_start: BlockCount,
    /// The root directory does not have a reserved area in FAT32. This is the
    /// cluster it starts in (nominally 2).
    pub(crate) first_root_dir_cluster: Cluster,
    /// Expected number of free clusters
    pub(crate) free_clusters_count: Option<u32>,
    /// Number of the next expected free cluster
    pub(crate) next_free_cluster: Option<Cluster>,
    /// Total number of clusters
    pub(crate) cluster_count: u32,
    /// Block idx of the info sector
    pub(crate) info_location: BlockIdx,
}

impl core::fmt::Debug for VolumeName {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        match core::str::from_utf8(&self.data) {
            Ok(s) => write!(fmt, "{:?}", s),
            Err(_e) => write!(fmt, "{:?}", &self.data),
        }
    }
}

struct Bpb<'a> {
    data: &'a [u8; 512],
    fat_type: FatType,
    cluster_count: u32,
}

impl<'a> Bpb<'a> {
    const FOOTER_VALUE: u16 = 0xAA55;

    pub fn create_from_bytes(data: &[u8; 512]) -> Result<Bpb, &'static str> {
        let mut bpb = Bpb {
            data,
            fat_type: FatType::Fat16,
            cluster_count: 0,
        };
        if bpb.footer() != Self::FOOTER_VALUE {
            return Err("Bad BPB footer");
        }

        let root_dir_blocks = ((u32::from(bpb.root_entries_count()) * OnDiskDirEntry::LEN_U32)
            + (Block::LEN_U32 - 1))
            / Block::LEN_U32;
        let data_blocks = bpb.total_blocks()
            - (u32::from(bpb.reserved_block_count())
                + (u32::from(bpb.num_fats()) * bpb.fat_size())
                + root_dir_blocks);
        bpb.cluster_count = data_blocks / u32::from(bpb.blocks_per_cluster());
        if bpb.cluster_count < 4085 {
            return Err("FAT12 is unsupported");
        } else if bpb.cluster_count < 65525 {
            bpb.fat_type = FatType::Fat16;
        } else {
            bpb.fat_type = FatType::Fat32;
        }

        match bpb.fat_type {
            FatType::Fat16 => Ok(bpb),
            FatType::Fat32 if bpb.fs_ver() == 0 => {
                // Only support FAT32 version 0.0
                Ok(bpb)
            }
            _ => Err("Invalid FAT format"),
        }
    }

    // FAT16/FAT32
    define_field!(bytes_per_block, u16, 11);
    define_field!(blocks_per_cluster, u8, 13);
    define_field!(reserved_block_count, u16, 14);
    define_field!(num_fats, u8, 16);
    define_field!(root_entries_count, u16, 17);
    define_field!(total_blocks16, u16, 19);
    define_field!(media, u8, 21);
    define_field!(fat_size16, u16, 22);
    define_field!(blocks_per_track, u16, 24);
    define_field!(num_heads, u16, 26);
    define_field!(hidden_blocks, u32, 28);
    define_field!(total_blocks32, u32, 32);
    define_field!(footer, u16, 510);

    // FAT32 only
    define_field!(fat_size32, u32, 36);
    define_field!(fs_ver, u16, 42);
    define_field!(first_root_dir_cluster, u32, 44);
    define_field!(fs_info, u16, 48);
    define_field!(backup_boot_block, u16, 50);

    pub fn oem_name(&self) -> &[u8] {
        &self.data[3..11]
    }

    // FAT16/FAT32 functions

    pub fn drive_number(&self) -> u8 {
        if self.fat_type != FatType::Fat32 {
            self.data[36]
        } else {
            self.data[64]
        }
    }

    pub fn boot_signature(&self) -> u8 {
        if self.fat_type != FatType::Fat32 {
            self.data[38]
        } else {
            self.data[66]
        }
    }

    pub fn volume_id(&self) -> u32 {
        if self.fat_type != FatType::Fat32 {
            LittleEndian::read_u32(&self.data[39..=42])
        } else {
            LittleEndian::read_u32(&self.data[67..=70])
        }
    }

    pub fn volume_label(&self) -> &[u8] {
        if self.fat_type != FatType::Fat32 {
            &self.data[43..=53]
        } else {
            &self.data[71..=81]
        }
    }

    pub fn fs_type(&self) -> &[u8] {
        if self.fat_type != FatType::Fat32 {
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

    pub fn root_cluster(&self) -> Cluster {
        Cluster(LittleEndian::read_u32(&self.data[44..=47]))
    }

    pub fn fs_info_block(&self) -> Option<BlockCount> {
        if self.fat_type != FatType::Fat32 {
            None
        } else {
            Some(BlockCount(u32::from(self.fs_info())))
        }
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

    pub fn total_blocks(&self) -> u32 {
        let result = self.total_blocks16() as u32;
        if result != 0 {
            result
        } else {
            self.total_blocks32()
        }
    }

    pub fn total_clusters(&self) -> u32 {
        self.cluster_count
    }
}

/// File System Information structure is only present on FAT32 partitions. It may contain a valid
/// number of free clusters and the number of the next free cluster.
/// The information contained in the structure must be considered as advisory only.
/// File system driver implementations are not required to ensure that information within the
/// structure is kept consistent.
struct InfoSector<'a> {
    data: &'a [u8; 512],
}

impl<'a> InfoSector<'a> {
    const LEAD_SIG: u32 = 0x41615252;
    const STRUC_SIG: u32 = 0x61417272;
    const TRAIL_SIG: u32 = 0xAA550000;

    fn create_from_bytes(data: &[u8; 512]) -> Result<InfoSector, &'static str> {
        let info = InfoSector { data };
        if info.lead_sig() != Self::LEAD_SIG {
            return Err("Bad lead signature on InfoSector");
        }
        if info.struc_sig() != Self::STRUC_SIG {
            return Err("Bad struc signature on InfoSector");
        }
        if info.trail_sig() != Self::TRAIL_SIG {
            return Err("Bad trail signature on InfoSector");
        }
        Ok(info)
    }

    define_field!(lead_sig, u32, 0);
    define_field!(struc_sig, u32, 484);
    define_field!(free_count, u32, 488);
    define_field!(next_free, u32, 492);
    define_field!(trail_sig, u32, 508);

    pub fn free_clusters_count(&self) -> Option<u32> {
        match self.free_count() {
            0xFFFF_FFFF => None,
            n => Some(n),
        }
    }

    pub fn next_free_cluster(&self) -> Option<Cluster> {
        match self.next_free() {
            // 0 and 1 are reserved clusters
            0xFFFF_FFFF | 0 | 1 => None,
            n => Some(Cluster(n)),
        }
    }
}

pub(crate) struct OnDiskDirEntry<'a> {
    data: &'a [u8],
}

impl<'a> core::fmt::Debug for OnDiskDirEntry<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "OnDiskDirEntry<")?;
        write!(f, "raw_attr = {}", self.raw_attr())?;
        write!(f, ", create_time = {}", self.create_time())?;
        write!(f, ", create_date = {}", self.create_date())?;
        write!(f, ", last_access_data = {}", self.last_access_data())?;
        write!(f, ", first_cluster_hi = {}", self.first_cluster_hi())?;
        write!(f, ", write_time = {}", self.write_time())?;
        write!(f, ", write_date = {}", self.write_date())?;
        write!(f, ", first_cluster_lo = {}", self.first_cluster_lo())?;
        write!(f, ", file_size = {}", self.file_size())?;
        write!(f, ", is_end = {}", self.is_end())?;
        write!(f, ", is_valid = {}", self.is_valid())?;
        write!(f, ", is_lfn = {}", self.is_lfn())?;
        write!(
            f,
            ", first_cluster_fat32 = {:?}",
            self.first_cluster_fat32()
        )?;
        write!(
            f,
            ", first_cluster_fat16 = {:?}",
            self.first_cluster_fat16()
        )?;
        write!(f, ">")?;
        Ok(())
    }
}

/// Represents the 32 byte directory entry. This is the same for FAT16 and
/// FAT32 (except FAT16 doesn't use first_cluster_hi).
impl<'a> OnDiskDirEntry<'a> {
    pub(crate) const LEN: usize = 32;
    pub(crate) const LEN_U32: u32 = 32;
    const LFN_FRAGMENT_LEN: usize = 13;

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

    fn lfn_contents(&self) -> Option<(bool, u8, [char; 13])> {
        if self.is_lfn() {
            let mut buffer = [' '; 13];
            let is_start = (self.data[0] & 0x40) != 0;
            let sequence = self.data[0] & 0x1F;
            buffer[0] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[1..=2]) as u32).unwrap();
            buffer[1] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[3..=4]) as u32).unwrap();
            buffer[2] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[5..=6]) as u32).unwrap();
            buffer[3] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[7..=8]) as u32).unwrap();
            buffer[4] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[9..=10]) as u32).unwrap();
            buffer[5] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[14..=15]) as u32).unwrap();
            buffer[6] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[16..=17]) as u32).unwrap();
            buffer[7] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[18..=19]) as u32).unwrap();
            buffer[8] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[20..=21]) as u32).unwrap();
            buffer[9] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[22..=23]) as u32).unwrap();
            buffer[10] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[24..=25]) as u32).unwrap();
            buffer[11] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[28..=29]) as u32).unwrap();
            buffer[12] =
                core::char::from_u32(LittleEndian::read_u16(&self.data[30..=31]) as u32).unwrap();
            Some((is_start, sequence, buffer))
        } else {
            None
        }
    }

    fn matches(&self, sfn: &ShortFileName) -> bool {
        self.data[0..11] == sfn.contents
    }

    fn first_cluster_fat32(&self) -> Cluster {
        let cluster_no =
            ((self.first_cluster_hi() as u32) << 16) | (self.first_cluster_lo() as u32);
        Cluster(cluster_no)
    }

    fn first_cluster_fat16(&self) -> Cluster {
        let cluster_no = self.first_cluster_lo() as u32;
        Cluster(cluster_no)
    }

    fn get_entry(&self, fat_type: FatType, entry_block: BlockIdx, entry_offset: u32) -> DirEntry {
        let mut result = DirEntry {
            name: ShortFileName {
                contents: [0u8; 11],
            },
            mtime: Timestamp::from_fat(self.write_date(), self.write_time()),
            ctime: Timestamp::from_fat(self.create_date(), self.create_time()),
            attributes: Attributes::create_from_fat(self.raw_attr()),
            cluster: if fat_type == FatType::Fat32 {
                self.first_cluster_fat32()
            } else {
                self.first_cluster_fat16()
            },
            size: self.file_size(),
            entry_block,
            entry_offset,
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
        cluster: Cluster,
    ) -> Result<Cluster, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        let fat_offset = cluster.0 * 2;
        let this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
        let this_fat_ent_offset = (fat_offset % Block::LEN_U32) as usize;
        controller
            .block_device
            .read(&mut blocks, this_fat_block_num, "read_fat")
            .map_err(Error::DeviceError)?;
        let entry =
            LittleEndian::read_u16(&blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 1]);
        Ok(Cluster(u32::from(entry)))
    }

    /// Write a new entry in the FAT
    fn update_fat<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        cluster: Cluster,
        new_value: Cluster,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        let fat_offset = cluster.0 * 2;
        let this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
        let this_fat_ent_offset = (fat_offset % Block::LEN_U32) as usize;
        controller
            .block_device
            .read(&mut blocks, this_fat_block_num, "read_fat")
            .map_err(Error::DeviceError)?;
        let entry = match new_value {
            Cluster::INVALID => 0xFFF6,
            Cluster::BAD => 0xFFF7,
            Cluster::EMPTY => 0x0000,
            Cluster::END_OF_FILE => 0xFFFF,
            _ => new_value.0 as u16,
        };
        LittleEndian::write_u16(
            &mut blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 1],
            entry,
        );
        controller
            .block_device
            .write(&blocks, this_fat_block_num)
            .map_err(Error::DeviceError)?;
        Ok(())
    }

    /// Look in the FAT to see which cluster comes next.
    pub(crate) fn next_cluster<D, T>(
        &self,
        controller: &Controller<D, T>,
        cluster: Cluster,
    ) -> Result<Cluster, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        let fat_offset = cluster.0 * 2;
        let this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
        let this_fat_ent_offset = (fat_offset % Block::LEN_U32) as usize;
        controller
            .block_device
            .read(&mut blocks, this_fat_block_num, "next_cluster")
            .map_err(Error::DeviceError)?;
        let fat_entry =
            LittleEndian::read_u16(&blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 1]);
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
                Ok(Cluster(u32::from(f)))
            }
        }
    }

    /// Number of bytes in a cluster.
    pub(crate) fn bytes_per_cluster(&self) -> u32 {
        u32::from(self.blocks_per_cluster) * Block::LEN_U32
    }

    /// Converts a cluster number (or `Cluster`) to a block number (or
    /// `BlockIdx`). Gives an absolute `BlockIdx` you can pass to the
    /// controller.
    pub(crate) fn cluster_to_block(&self, cluster: Cluster) -> BlockIdx {
        self.lba_start
            + match cluster {
                Cluster::ROOT_DIR => self.first_root_dir_block,
                Cluster(c) => {
                    // FirstSectorofCluster = ((N – 2) * BPB_SecPerClus) + FirstDataSector;
                    let first_block_of_cluster =
                        BlockCount((c - 2) * u32::from(self.blocks_per_cluster));
                    self.first_data_block + first_block_of_cluster
                }
            }
    }

    /// Get an entry from the given directory
    pub(crate) fn find_directory_entry<D, T>(
        &self,
        controller: &mut Controller<D, T>,
        dir: &Directory,
        name: &str,
    ) -> Result<DirEntry, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let match_name = ShortFileName::create_from_str(name).map_err(Error::FilenameError)?;
        let mut first_dir_block_num = match dir.cluster {
            Cluster::ROOT_DIR => self.lba_start + self.first_root_dir_block,
            _ => self.cluster_to_block(dir.cluster),
        };
        let mut current_cluster = Some(dir.cluster);
        let mut blocks = [Block::new()];

        let dir_size = match dir.cluster {
            Cluster::ROOT_DIR => BlockCount(
                ((self.root_entries_count as u32 * 32) + (Block::LEN as u32 - 1))
                    / Block::LEN as u32,
            ),
            _ => BlockCount(u32::from(self.blocks_per_cluster)),
        };
        while let Some(cluster) = current_cluster {
            for block in first_dir_block_num.range(dir_size) {
                controller
                    .block_device
                    .read(&mut blocks, block, "read_dir")
                    .map_err(Error::DeviceError)?;
                for entry in 0..Block::LEN / OnDiskDirEntry::LEN {
                    let start = entry * OnDiskDirEntry::LEN;
                    let end = (entry + 1) * OnDiskDirEntry::LEN;
                    let dir_entry = OnDiskDirEntry::new(&blocks[0][start..end]);
                    if dir_entry.is_end() {
                        // Can quit early
                        return Err(Error::FileNotFound);
                    } else if dir_entry.matches(&match_name) {
                        // Found it
                        // Safe, since Block::LEN always fits on a u32
                        let start = u32::try_from(start).unwrap();
                        return Ok(dir_entry.get_entry(FatType::Fat16, block, start));
                    }
                }
            }
            if cluster != Cluster::ROOT_DIR {
                current_cluster = match self.next_cluster(controller, cluster) {
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
        Err(Error::FileNotFound)
    }

    /// Finds a empty entry space and writes the new entry to it, allocates a new cluster if it's
    /// needed
    pub(crate) fn write_new_directory_entry<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        dir: &Directory,
        name: ShortFileName,
        attributes: Attributes,
    ) -> Result<(DirEntry), Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut first_dir_block_num = match dir.cluster {
            Cluster::ROOT_DIR => self.lba_start + self.first_root_dir_block,
            _ => self.cluster_to_block(dir.cluster),
        };
        let mut current_cluster = Some(dir.cluster);
        let mut blocks = [Block::new()];

        let dir_size = match dir.cluster {
            Cluster::ROOT_DIR => BlockCount(
                ((self.root_entries_count as u32 * 32) + (Block::LEN as u32 - 1))
                    / Block::LEN as u32,
            ),
            _ => BlockCount(u32::from(self.blocks_per_cluster)),
        };
        while let Some(cluster) = current_cluster {
            for block in first_dir_block_num.range(dir_size) {
                controller
                    .block_device
                    .read(&mut blocks, block, "read_dir")
                    .map_err(Error::DeviceError)?;
                for entry in 0..Block::LEN / OnDiskDirEntry::LEN {
                    let start = entry * OnDiskDirEntry::LEN;
                    let end = (entry + 1) * OnDiskDirEntry::LEN;
                    let dir_entry = OnDiskDirEntry::new(&blocks[0][start..end]);
                    // 0x00 or 0xE5 represents a free entry
                    if !dir_entry.is_valid() {
                        let ctime = controller.timesource.get_timestamp();
                        let entry =
                            DirEntry::new(name, attributes, Cluster(0), ctime, block, start as u32);
                        &blocks[0][start..start + 32]
                            .copy_from_slice(&entry.serialize(FatType::Fat16)[..]);
                        controller
                            .block_device
                            .write(&mut blocks, block)
                            .map_err(Error::DeviceError)?;
                        return Ok(entry);
                    }
                }
            }
            if cluster != Cluster::ROOT_DIR {
                current_cluster = match self.next_cluster(controller, cluster) {
                    Ok(n) => {
                        first_dir_block_num = self.cluster_to_block(n);
                        Some(n)
                    }
                    Err(Error::EndOfFile) => {
                        let c = self.alloc_cluster(controller, Some(cluster), true)?;
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

    /// Calls callback `func` with every valid entry in the given directory.
    /// Useful for performing directory listings.
    pub(crate) fn iterate_dir<D, T, F>(
        &self,
        controller: &Controller<D, T>,
        dir: &Directory,
        mut func: F,
    ) -> Result<(), Error<D::Error>>
    where
        F: FnMut(&DirEntry),
        D: BlockDevice,
        T: TimeSource,
    {
        let mut first_dir_block_num = match dir.cluster {
            Cluster::ROOT_DIR => self.lba_start + self.first_root_dir_block,
            _ => self.cluster_to_block(dir.cluster),
        };
        let mut current_cluster = Some(dir.cluster);
        // TODO track actual directory size
        let dir_size = match dir.cluster {
            Cluster::ROOT_DIR => BlockCount(
                ((self.root_entries_count as u32 * 32) + (Block::LEN as u32 - 1))
                    / Block::LEN as u32,
            ),
            _ => BlockCount(u32::from(self.blocks_per_cluster)),
        };
        let mut blocks = [Block::new()];
        while let Some(cluster) = current_cluster {
            for block in first_dir_block_num.range(dir_size) {
                controller
                    .block_device
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
                        let entry = dir_entry.get_entry(FatType::Fat16, block, start);
                        func(&entry);
                    }
                }
            }
            if cluster != Cluster::ROOT_DIR {
                current_cluster = match self.next_cluster(controller, cluster) {
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

    // TODO write some tests
    /// Finds the next free cluster after the start_cluster and before end_cluster
    pub(crate) fn find_next_free_cluster<D, T>(
        &self,
        controller: &mut Controller<D, T>,
        start_cluster: Cluster,
        end_cluster: Cluster,
    ) -> Result<Cluster, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        let mut current_cluster = start_cluster + 1;
        while current_cluster.0 < end_cluster.0 {
            let fat_offset = current_cluster.0 * 2;
            let this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
            let mut this_fat_ent_offset =
                usize::try_from(fat_offset % Block::LEN_U32).map_err(|_| Error::ConversionError)?;
            controller
                .block_device
                .read(&mut blocks, this_fat_block_num, "next_cluster")
                .map_err(Error::DeviceError)?;

            while this_fat_ent_offset < Block::LEN - 2 {
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
        Err(Error::NotEnoughSpace)
    }

    /// Tries to allocate a cluster
    pub(crate) fn alloc_cluster<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        prev_cluster: Option<Cluster>,
        zero: bool,
    ) -> Result<Cluster, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let end_cluster = Cluster(self.cluster_count + RESERVED_ENTRIES);
        let start_cluster = match self.next_free_cluster {
            Some(cluster) if cluster.0 < end_cluster.0 => cluster,
            _ => Cluster(RESERVED_ENTRIES),
        };
        let new_cluster = match self.find_next_free_cluster(controller, start_cluster, end_cluster)
        {
            Ok(cluster) => cluster,
            Err(_) if start_cluster.0 > RESERVED_ENTRIES => {
                self.find_next_free_cluster(controller, Cluster(RESERVED_ENTRIES), end_cluster)?
            }
            Err(e) => return Err(e),
        };
        self.update_fat(controller, new_cluster, Cluster::END_OF_FILE)?;
        if let Some(cluster) = prev_cluster {
            self.update_fat(controller, cluster, new_cluster)?;
        }
        self.next_free_cluster = Some(new_cluster + 1);
        if let Some(ref mut number_free_cluster) = self.free_clusters_count {
            *number_free_cluster -= 1;
        };
        if zero {
            let blocks = [Block::new()];
            let first_block = self.cluster_to_block(new_cluster);
            let num_blocks = BlockCount(u32::from(self.blocks_per_cluster));
            for block in first_block.range(num_blocks) {
                controller
                    .block_device
                    .write(&blocks, block)
                    .map_err(Error::DeviceError)?;
            }
        }
        Ok(new_cluster)
    }

    /// Tries to allocate a chain of clusters
    pub(crate) fn alloc_clusters<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        mut prev_cluster: Option<Cluster>,
        mut clusters_to_alloc: u32,
        zero: bool,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        while clusters_to_alloc > 0 {
            let new_cluster = self.alloc_cluster(controller, prev_cluster, zero)?;
            prev_cluster = Some(new_cluster);
            clusters_to_alloc -= 1;
        }
        Ok(())
    }

    /// Marks the input cluster as an EOF and the subsequents clusters in the chain as free
    pub(crate) fn truncate_cluster_chain<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        cluster: Cluster,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut next = match self.next_cluster(controller, cluster) {
            Ok(n) => n,
            Err(Error::EndOfFile) => return Ok(()),
            Err(e) => return Err(e),
        };
        if let Some(ref mut next_free_cluster) = self.next_free_cluster {
            if next_free_cluster.0 > next.0 {
                *next_free_cluster = next;
            }
        } else {
            self.next_free_cluster = Some(next);
        }
        self.update_fat(controller, cluster, Cluster::END_OF_FILE)?;

        loop {
            match self.next_cluster(controller, next) {
                Ok(n) => {
                    self.update_fat(controller, next, Cluster::EMPTY)?;
                    next = n;
                }
                Err(Error::EndOfFile) => {
                    self.update_fat(controller, next, Cluster::EMPTY)?;
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
}

impl Fat32Volume {
    /// Get an entry from the FAT
    fn get_fat<D, T>(
        &self,
        controller: &mut Controller<D, T>,
        cluster: Cluster,
    ) -> Result<Cluster, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        // FAT32 => 4 bytes per entry
        let fat_offset = cluster.0 as u32 * 4;
        let this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
        let this_fat_ent_offset = (fat_offset % Block::LEN_U32) as usize;
        controller
            .block_device
            .read(&mut blocks, this_fat_block_num, "read_fat")
            .map_err(Error::DeviceError)?;
        let mut entry =
            LittleEndian::read_u32(&blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 3]);
        entry &= 0x0FFF_FFFF;
        Ok(Cluster(entry))
    }

    /// Write a new entry in the FAT
    fn update_fat<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        cluster: Cluster,
        new_value: Cluster,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        // FAT32 => 4 bytes per entry
        let fat_offset = cluster.0 as u32 * 4;
        let this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
        let this_fat_ent_offset = (fat_offset % Block::LEN_U32) as usize;
        controller
            .block_device
            .read(&mut blocks, this_fat_block_num, "read_fat")
            .map_err(Error::DeviceError)?;
        let entry = match new_value {
            Cluster::INVALID => 0x0FFF_FFF6,
            Cluster::BAD => 0x0FFF_FFF7,
            Cluster::EMPTY => 0x0000_0000,
            _ => new_value.0,
        };
        let existing =
            LittleEndian::read_u32(&blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 3]);
        let new = (existing & 0xF000_0000) | (entry & 0x0FFF_FFFF);
        LittleEndian::write_u32(
            &mut blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 3],
            new,
        );
        controller
            .block_device
            .write(&blocks, this_fat_block_num)
            .map_err(Error::DeviceError)?;
        Ok(())
    }

    /// Look in the FAT to see which cluster comes next.
    pub(crate) fn next_cluster<D, T>(
        &self,
        controller: &Controller<D, T>,
        cluster: Cluster,
    ) -> Result<Cluster, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        let fat_offset = cluster.0 * 4;
        let this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
        let this_fat_ent_offset = (fat_offset % Block::LEN_U32) as usize;
        controller
            .block_device
            .read(&mut blocks, this_fat_block_num, "next_cluster")
            .map_err(Error::DeviceError)?;
        let fat_entry =
            LittleEndian::read_u32(&blocks[0][this_fat_ent_offset..=this_fat_ent_offset + 3])
                & 0x0FFF_FFFF;
        match fat_entry {
            0x0000_0000 => {
                // Jumped to free space
                Err(Error::JumpedFree)
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
                Ok(Cluster(u32::from(f)))
            }
        }
    }

    /// Number of bytes in a cluster.
    pub(crate) fn bytes_per_cluster(&self) -> u32 {
        u32::from(self.blocks_per_cluster) * Block::LEN_U32
    }

    /// Converts a cluster number (or `Cluster`) to a block number (or
    /// `BlockIdx`). Gives an absolute `BlockIdx` you can pass to the
    /// controller.
    pub(crate) fn cluster_to_block(&self, cluster: Cluster) -> BlockIdx {
        let cluster_num = match cluster {
            Cluster::ROOT_DIR => self.first_root_dir_cluster.0,
            c => c.0,
        };
        // FirstSectorofCluster = ((N – 2) * BPB_SecPerClus) + FirstDataSector;
        let first_block_of_cluster =
            BlockCount((cluster_num - 2) * u32::from(self.blocks_per_cluster));
        self.lba_start + self.first_data_block + first_block_of_cluster
    }

    /// Get an entry from the given directory
    pub(crate) fn find_directory_entry<D, T>(
        &self,
        controller: &mut Controller<D, T>,
        dir: &Directory,
        name: &str,
    ) -> Result<DirEntry, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let match_name = ShortFileName::create_from_str(name).map_err(Error::FilenameError)?;
        let mut current_cluster = match dir.cluster {
            Cluster::ROOT_DIR => Some(self.first_root_dir_cluster),
            _ => Some(dir.cluster),
        };
        let mut blocks = [Block::new()];
        while let Some(cluster) = current_cluster {
            let block_idx = self.cluster_to_block(cluster);
            for block in block_idx.range(BlockCount(u32::from(self.blocks_per_cluster))) {
                controller
                    .block_device
                    .read(&mut blocks, block, "read_dir")
                    .map_err(Error::DeviceError)?;
                for entry in 0..Block::LEN / OnDiskDirEntry::LEN {
                    let start = entry * OnDiskDirEntry::LEN;
                    let end = (entry + 1) * OnDiskDirEntry::LEN;
                    let dir_entry = OnDiskDirEntry::new(&blocks[0][start..end]);
                    if dir_entry.is_end() {
                        // Can quit early
                        return Err(Error::FileNotFound);
                    } else if dir_entry.matches(&match_name) {
                        // Found it
                        // Safe, since Block::LEN always fits on a u32
                        let start = u32::try_from(start).unwrap();
                        return Ok(dir_entry.get_entry(FatType::Fat16, block, start));
                    }
                }
            }
            current_cluster = match self.next_cluster(controller, cluster) {
                Ok(n) => Some(n),
                _ => None,
            }
        }
        Err(Error::FileNotFound)
    }

    /// Finds a empty entry space and writes the new entry to it, allocates a new cluster if it's
    /// needed
    pub(crate) fn write_new_directory_entry<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        dir: &Directory,
        name: ShortFileName,
        attributes: Attributes,
    ) -> Result<(DirEntry), Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut first_dir_block_num = match dir.cluster {
            Cluster::ROOT_DIR => self.cluster_to_block(self.first_root_dir_cluster),
            _ => self.cluster_to_block(dir.cluster),
        };
        let mut current_cluster = Some(dir.cluster);
        let mut blocks = [Block::new()];

        let dir_size = BlockCount(u32::from(self.blocks_per_cluster));
        while let Some(cluster) = current_cluster {
            for block in first_dir_block_num.range(dir_size) {
                controller
                    .block_device
                    .read(&mut blocks, block, "read_dir")
                    .map_err(Error::DeviceError)?;
                for entry in 0..Block::LEN / OnDiskDirEntry::LEN {
                    let start = entry * OnDiskDirEntry::LEN;
                    let end = (entry + 1) * OnDiskDirEntry::LEN;
                    let dir_entry = OnDiskDirEntry::new(&blocks[0][start..end]);
                    // 0x00 or 0xE5 represents a free entry
                    if !dir_entry.is_valid() {
                        let ctime = controller.timesource.get_timestamp();
                        let entry =
                            DirEntry::new(name, attributes, Cluster(0), ctime, block, start as u32);
                        &blocks[0][start..start + 32]
                            .copy_from_slice(&entry.serialize(FatType::Fat32)[..]);
                        controller
                            .block_device
                            .write(&mut blocks, block)
                            .map_err(Error::DeviceError)?;
                        return Ok(entry);
                    }
                }
            }
            current_cluster = match self.next_cluster(controller, cluster) {
                Ok(n) => {
                    first_dir_block_num = self.cluster_to_block(n);
                    Some(n)
                }
                Err(Error::EndOfFile) => {
                    let c = self.alloc_cluster(controller, Some(cluster), true)?;
                    first_dir_block_num = self.cluster_to_block(c);
                    Some(c)
                }
                _ => None,
            };
        }
        Err(Error::NotEnoughSpace)
    }

    /// Calls callback `func` with every valid entry in the given directory.
    /// Useful for performing directory listings.
    pub(crate) fn iterate_dir<D, T, F>(
        &self,
        controller: &Controller<D, T>,
        dir: &Directory,
        mut func: F,
    ) -> Result<(), Error<D::Error>>
    where
        F: FnMut(&DirEntry),
        D: BlockDevice,
        T: TimeSource,
    {
        let mut current_cluster = match dir.cluster {
            Cluster::ROOT_DIR => Some(self.first_root_dir_cluster),
            _ => Some(dir.cluster),
        };
        let mut blocks = [Block::new()];
        while let Some(cluster) = current_cluster {
            let block_idx = self.cluster_to_block(cluster);
            for block in block_idx.range(BlockCount(u32::from(self.blocks_per_cluster))) {
                controller
                    .block_device
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
                        let entry = dir_entry.get_entry(FatType::Fat16, block, start);
                        func(&entry);
                    }
                }
            }
            current_cluster = match self.next_cluster(controller, cluster) {
                Ok(n) => Some(n),
                _ => None,
            };
        }
        Ok(())
    }

    // TODO write some tests
    /// Finds the next free cluster after the start_cluster and before end_cluster
    pub(crate) fn find_next_free_cluster<D, T>(
        &self,
        controller: &mut Controller<D, T>,
        start_cluster: Cluster,
        end_cluster: Cluster,
    ) -> Result<Cluster, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut blocks = [Block::new()];
        let mut current_cluster = start_cluster + 1;
        while current_cluster.0 < end_cluster.0 {
            let fat_offset = current_cluster.0 * 4;
            let this_fat_block_num = self.lba_start + self.fat_start.offset_bytes(fat_offset);
            let mut this_fat_ent_offset =
                usize::try_from(fat_offset % Block::LEN_U32).map_err(|_| Error::ConversionError)?;
            controller
                .block_device
                .read(&mut blocks, this_fat_block_num, "next_cluster")
                .map_err(Error::DeviceError)?;

            while this_fat_ent_offset < Block::LEN - 4 {
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
        Err(Error::NotEnoughSpace)
    }

    /// Tries to allocate a cluster
    pub(crate) fn alloc_cluster<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        prev_cluster: Option<Cluster>,
        zero: bool,
    ) -> Result<Cluster, Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let end_cluster = Cluster(self.cluster_count + RESERVED_ENTRIES);
        let start_cluster = match self.next_free_cluster {
            Some(cluster) if cluster.0 < end_cluster.0 => cluster,
            _ => Cluster(RESERVED_ENTRIES),
        };
        let new_cluster = match self.find_next_free_cluster(controller, start_cluster, end_cluster)
        {
            Ok(cluster) => cluster,
            Err(_) if start_cluster.0 > RESERVED_ENTRIES => {
                self.find_next_free_cluster(controller, Cluster(RESERVED_ENTRIES), end_cluster)?
            }
            Err(e) => return Err(e),
        };
        if let Some(cluster) = prev_cluster {
            self.update_fat(controller, cluster, new_cluster)?;
        }
        self.update_fat(controller, new_cluster, Cluster::END_OF_FILE)?;
        self.next_free_cluster =
            match self.find_next_free_cluster(controller, new_cluster, end_cluster) {
                Ok(cluster) => Some(cluster),
                Err(_) if new_cluster.0 > RESERVED_ENTRIES => {
                    match self.find_next_free_cluster(
                        controller,
                        Cluster(RESERVED_ENTRIES),
                        end_cluster,
                    ) {
                        Ok(cluster) => Some(cluster),
                        Err(e) => return Err(e),
                    }
                }
                Err(e) => return Err(e),
            };
        if let Some(ref mut number_free_cluster) = self.free_clusters_count {
            *number_free_cluster -= 1;
        };
        if zero {
            let blocks = [Block::new()];
            let first_block = self.cluster_to_block(new_cluster);
            let num_blocks = BlockCount(u32::from(self.blocks_per_cluster));
            for block in first_block.range(num_blocks) {
                controller
                    .block_device
                    .write(&blocks, block)
                    .map_err(Error::DeviceError)?;
            }
        }
        Ok(new_cluster)
    }

    /// Tries to allocate a chain of clusters
    pub(crate) fn alloc_clusters<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        mut prev_cluster: Option<Cluster>,
        mut clusters_to_alloc: u32,
        zero: bool,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        while clusters_to_alloc > 0 {
            let new_cluster = self.alloc_cluster(controller, prev_cluster, zero)?;
            prev_cluster = Some(new_cluster);
            clusters_to_alloc -= 1;
        }
        Ok(())
    }

    /// Marks the input cluster as an EOF and the subsequents clusters in the chain as free
    pub(crate) fn truncate_cluster_chain<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
        cluster: Cluster,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        let mut next = match self.next_cluster(controller, cluster) {
            Ok(n) => n,
            Err(Error::EndOfFile) => return Ok(()),
            Err(e) => return Err(e),
        };
        if let Some(ref mut next_free_cluster) = self.next_free_cluster {
            if next_free_cluster.0 > next.0 {
                *next_free_cluster = next;
            }
        } else {
            self.next_free_cluster = Some(next);
        }
        self.update_fat(controller, cluster, Cluster::END_OF_FILE)?;

        loop {
            match self.next_cluster(controller, next) {
                Ok(n) => {
                    self.update_fat(controller, next, Cluster::EMPTY)?;
                    next = n;
                }
                Err(Error::EndOfFile) => {
                    self.update_fat(controller, next, Cluster::EMPTY)?;
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

    /// Writes the updated info_sector to disk
    pub(crate) fn update_info_sector<D, T>(
        &mut self,
        controller: &mut Controller<D, T>,
    ) -> Result<(), Error<D::Error>>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        if self.free_clusters_count.is_none() && self.next_free_cluster.is_none() {
            return Ok(());
        }
        let mut blocks = [Block::new()];
        controller
            .block_device
            .read(&mut blocks, self.info_location, "read_info_sector")
            .map_err(Error::DeviceError)?;
        let block = &mut blocks[0];
        if let Some(count) = self.free_clusters_count {
            block[488..492].copy_from_slice(&count.to_le_bytes());
        }
        if let Some(next_free_cluster) = self.next_free_cluster {
            block[492..496].copy_from_slice(&next_free_cluster.0.to_le_bytes());
        }
        controller
            .block_device
            .write(&mut blocks, self.info_location)
            .map_err(Error::DeviceError)?;
        Ok(())
    }
}

/// Load the boot parameter block from the start of the given partition and
/// determine if the partition contains a valid FAT16 or FAT32 file system.
pub fn parse_volume<D, T>(
    controller: &mut Controller<D, T>,
    lba_start: BlockIdx,
    num_blocks: BlockCount,
) -> Result<VolumeType, Error<D::Error>>
where
    D: BlockDevice,
    T: TimeSource,
    D::Error: core::fmt::Debug,
{
    let mut blocks = [Block::new()];
    controller
        .block_device
        .read(&mut blocks, lba_start, "read_bpb")
        .map_err(Error::DeviceError)?;
    let block = &blocks[0];
    let bpb = Bpb::create_from_bytes(&block).map_err(Error::FormatError)?;
    match bpb.fat_type {
        FatType::Fat16 => {
            // FirstDataSector = BPB_ResvdSecCnt + (BPB_NumFATs * FATSz) + RootDirSectors;
            let root_dir_blocks = ((u32::from(bpb.root_entries_count()) * OnDiskDirEntry::LEN_U32)
                + (Block::LEN_U32 - 1))
                / Block::LEN_U32;
            let fat_start = BlockCount(u32::from(bpb.reserved_block_count()));
            let first_root_dir_block =
                fat_start + BlockCount(u32::from(bpb.num_fats()) * bpb.fat_size());
            let first_data_block = first_root_dir_block + BlockCount(root_dir_blocks);
            let mut volume = Fat16Volume {
                lba_start,
                num_blocks,
                name: VolumeName { data: [0u8; 11] },
                root_entries_count: bpb.root_entries_count(),
                blocks_per_cluster: bpb.blocks_per_cluster(),
                first_root_dir_block: (first_root_dir_block),
                first_data_block: (first_data_block),
                fat_start: BlockCount(u32::from(bpb.reserved_block_count())),
                free_clusters_count: None,
                next_free_cluster: None,
                cluster_count: bpb.total_clusters(),
            };
            volume.name.data[..].copy_from_slice(bpb.volume_label());
            Ok(VolumeType::Fat16(volume))
        }
        FatType::Fat32 => {
            // FirstDataSector = BPB_ResvdSecCnt + (BPB_NumFATs * FATSz);
            let first_data_block = u32::from(bpb.reserved_block_count())
                + (u32::from(bpb.num_fats()) * bpb.fat_size());

            // Safe to unwrap since this is a Fat32 Type
            let info_location = bpb.fs_info_block().unwrap();
            let mut info_blocks = [Block::new()];
            controller
                .block_device
                .read(
                    &mut info_blocks,
                    lba_start + info_location,
                    "read_info_sector",
                )
                .map_err(Error::DeviceError)?;
            let info_block = &info_blocks[0];
            let info_sector =
                InfoSector::create_from_bytes(&info_block).map_err(Error::FormatError)?;

            let mut volume = Fat32Volume {
                lba_start,
                num_blocks,
                name: VolumeName { data: [0u8; 11] },
                blocks_per_cluster: bpb.blocks_per_cluster(),
                first_data_block: BlockCount(first_data_block),
                fat_start: BlockCount(u32::from(bpb.reserved_block_count())),
                first_root_dir_cluster: Cluster(bpb.first_root_dir_cluster()),
                free_clusters_count: info_sector.free_clusters_count(),
                next_free_cluster: info_sector.next_free_cluster(),
                cluster_count: bpb.total_clusters(),
                info_location: lba_start + info_location,
            };
            volume.name.data[..].copy_from_slice(bpb.volume_label());
            Ok(VolumeType::Fat32(volume))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn parse(input: &str) -> Vec<u8> {
        let mut output = Vec::new();
        for line in input.lines() {
            let line = line.trim();
            if line.len() > 0 {
                // 32 bytes per line
                for index in 0..32 {
                    let start = index * 2;
                    let end = start + 1;
                    let piece = &line[start..=end];
                    let value = u8::from_str_radix(piece, 16).unwrap();
                    output.push(value);
                }
            }
        }
        output
    }

    /// This is the first block of this directory listing.
    /// total 19880
    /// -rw-r--r-- 1 jonathan jonathan   10841 2016-03-01 19:56:36.000000000 +0000  bcm2708-rpi-b.dtb
    /// -rw-r--r-- 1 jonathan jonathan   11120 2016-03-01 19:56:34.000000000 +0000  bcm2708-rpi-b-plus.dtb
    /// -rw-r--r-- 1 jonathan jonathan   10871 2016-03-01 19:56:36.000000000 +0000  bcm2708-rpi-cm.dtb
    /// -rw-r--r-- 1 jonathan jonathan   12108 2016-03-01 19:56:36.000000000 +0000  bcm2709-rpi-2-b.dtb
    /// -rw-r--r-- 1 jonathan jonathan   12575 2016-03-01 19:56:36.000000000 +0000  bcm2710-rpi-3-b.dtb
    /// -rw-r--r-- 1 jonathan jonathan   17920 2016-03-01 19:56:38.000000000 +0000  bootcode.bin
    /// -rw-r--r-- 1 jonathan jonathan     136 2015-11-21 20:28:30.000000000 +0000  cmdline.txt
    /// -rw-r--r-- 1 jonathan jonathan    1635 2015-11-21 20:28:30.000000000 +0000  config.txt
    /// -rw-r--r-- 1 jonathan jonathan   18693 2016-03-01 19:56:30.000000000 +0000  COPYING.linux
    /// -rw-r--r-- 1 jonathan jonathan    2505 2016-03-01 19:56:38.000000000 +0000  fixup_cd.dat
    /// -rw-r--r-- 1 jonathan jonathan    6481 2016-03-01 19:56:38.000000000 +0000  fixup.dat
    /// -rw-r--r-- 1 jonathan jonathan    9722 2016-03-01 19:56:38.000000000 +0000  fixup_db.dat
    /// -rw-r--r-- 1 jonathan jonathan    9724 2016-03-01 19:56:38.000000000 +0000  fixup_x.dat
    /// -rw-r--r-- 1 jonathan jonathan     110 2015-11-21 21:32:06.000000000 +0000  issue.txt
    /// -rw-r--r-- 1 jonathan jonathan 4046732 2016-03-01 19:56:40.000000000 +0000  kernel7.img
    /// -rw-r--r-- 1 jonathan jonathan 3963140 2016-03-01 19:56:38.000000000 +0000  kernel.img
    /// -rw-r--r-- 1 jonathan jonathan    1494 2016-03-01 19:56:34.000000000 +0000  LICENCE.broadcom
    /// -rw-r--r-- 1 jonathan jonathan   18974 2015-11-21 21:32:06.000000000 +0000  LICENSE.oracle
    /// drwxr-xr-x 2 jonathan jonathan    8192 2016-03-01 19:56:54.000000000 +0000  overlays
    /// -rw-r--r-- 1 jonathan jonathan  612472 2016-03-01 19:56:40.000000000 +0000  start_cd.elf
    /// -rw-r--r-- 1 jonathan jonathan 4888200 2016-03-01 19:56:42.000000000 +0000  start_db.elf
    /// -rw-r--r-- 1 jonathan jonathan 2739672 2016-03-01 19:56:40.000000000 +0000  start.elf
    /// -rw-r--r-- 1 jonathan jonathan 3840328 2016-03-01 19:56:44.000000000 +0000  start_x.elf
    /// drwxr-xr-x 2 jonathan jonathan    8192 2015-12-05 21:55:06.000000000 +0000 'System Volume Information'
    #[test]
    fn test_dir_entries() {
        #[derive(Debug)]
        enum Expected {
            Lfn(bool, u8, [char; 13]),
            Short(DirEntry),
        }
        let raw_data = r#"
        626f6f7420202020202020080000699c754775470000699c7547000000000000 boot       ...i.uGuG..i.uG......
        416f007600650072006c000f00476100790073000000ffffffff0000ffffffff Ao.v.e.r.l...Ga.y.s.............
        4f5645524c4159532020201000001b9f6148614800001b9f6148030000000000 OVERLAYS   .....aHaH....aH......
        422d0070006c00750073000f00792e006400740062000000ffff0000ffffffff B-.p.l.u.s...y..d.t.b...........
        01620063006d00320037000f0079300038002d0072007000690000002d006200 .b.c.m.2.7...y0.8.-.r.p.i...-.b.
        42434d3237307e31445442200064119f614861480000119f61480900702b0000 BCM270~1DTB .d..aHaH....aH..p+..
        4143004f005000590049000f00124e0047002e006c0069006e00000075007800 AC.O.P.Y.I....N.G...l.i.n...u.x.
        434f5059494e7e314c494e2000000f9f6148614800000f9f6148050005490000 COPYIN~1LIN ....aHaH....aH...I..
        4263006f006d000000ffff0f0067ffffffffffffffffffffffff0000ffffffff Bc.o.m.......g..................
        014c004900430045004e000f0067430045002e00620072006f00000061006400 .L.I.C.E.N...gC.E...b.r.o...a.d.
        4c4943454e437e3142524f200000119f614861480000119f61480800d6050000 LICENC~1BRO ....aHaH....aH......
        422d0062002e00640074000f001962000000ffffffffffffffff0000ffffffff B-.b...d.t....b.................
        01620063006d00320037000f0019300039002d0072007000690000002d003200 .b.c.m.2.7....0.9.-.r.p.i...-.2.
        42434d3237307e34445442200064129f614861480000129f61480f004c2f0000 BCM270~4DTB .d..aHaH....aH..L/..
        422e0064007400620000000f0059ffffffffffffffffffffffff0000ffffffff B..d.t.b.....Y..................
        01620063006d00320037000f0059300038002d0072007000690000002d006200 .b.c.m.2.7...Y0.8.-.r.p.i...-.b.
        "#;
        let results = [
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str_mixed_case("boot").unwrap(),
                mtime: Timestamp::from_calendar(2015, 11, 21, 19, 35, 18).unwrap(),
                ctime: Timestamp::from_calendar(2015, 11, 21, 19, 35, 18).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::VOLUME),
                cluster: Cluster(0),
                size: 0,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                1,
                [
                    'o', 'v', 'e', 'r', 'l', 'a', 'y', 's', '\u{0000}', '\u{ffff}', '\u{ffff}',
                    '\u{ffff}', '\u{ffff}',
                ],
            ),
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str("OVERLAYS").unwrap(),
                mtime: Timestamp::from_calendar(2016, 03, 01, 19, 56, 54).unwrap(),
                ctime: Timestamp::from_calendar(2016, 03, 01, 19, 56, 54).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::DIRECTORY),
                cluster: Cluster(3),
                size: 0,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                2,
                [
                    '-', 'p', 'l', 'u', 's', '.', 'd', 't', 'b', '\u{0000}', '\u{ffff}',
                    '\u{ffff}', '\u{ffff}',
                ],
            ),
            Expected::Lfn(
                false,
                1,
                [
                    'b', 'c', 'm', '2', '7', '0', '8', '-', 'r', 'p', 'i', '-', 'b',
                ],
            ),
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str("BCM270~1.DTB").unwrap(),
                mtime: Timestamp::from_calendar(2016, 03, 01, 19, 56, 34).unwrap(),
                ctime: Timestamp::from_calendar(2016, 03, 01, 19, 56, 34).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::ARCHIVE),
                cluster: Cluster(9),
                size: 11120,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                1,
                [
                    'C', 'O', 'P', 'Y', 'I', 'N', 'G', '.', 'l', 'i', 'n', 'u', 'x',
                ],
            ),
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str("COPYIN~1.LIN").unwrap(),
                mtime: Timestamp::from_calendar(2016, 03, 01, 19, 56, 30).unwrap(),
                ctime: Timestamp::from_calendar(2016, 03, 01, 19, 56, 30).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::ARCHIVE),
                cluster: Cluster(5),
                size: 18693,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                2,
                [
                    'c', 'o', 'm', '\u{0}', '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
                    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
                ],
            ),
            Expected::Lfn(
                false,
                1,
                [
                    'L', 'I', 'C', 'E', 'N', 'C', 'E', '.', 'b', 'r', 'o', 'a', 'd',
                ],
            ),
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str("LICENC~1.BRO").unwrap(),
                mtime: Timestamp::from_calendar(2016, 03, 01, 19, 56, 34).unwrap(),
                ctime: Timestamp::from_calendar(2016, 03, 01, 19, 56, 34).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::ARCHIVE),
                cluster: Cluster(8),
                size: 1494,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                2,
                [
                    '-', 'b', '.', 'd', 't', 'b', '\u{0000}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
                    '\u{ffff}', '\u{ffff}', '\u{ffff}',
                ],
            ),
            Expected::Lfn(
                false,
                1,
                [
                    'b', 'c', 'm', '2', '7', '0', '9', '-', 'r', 'p', 'i', '-', '2',
                ],
            ),
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str("BCM270~4.DTB").unwrap(),
                mtime: Timestamp::from_calendar(2016, 03, 01, 19, 56, 36).unwrap(),
                ctime: Timestamp::from_calendar(2016, 03, 01, 19, 56, 36).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::ARCHIVE),
                cluster: Cluster(15),
                size: 12108,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                2,
                [
                    '.', 'd', 't', 'b', '\u{0000}', '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
                    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
                ],
            ),
            Expected::Lfn(
                false,
                1,
                [
                    'b', 'c', 'm', '2', '7', '0', '8', '-', 'r', 'p', 'i', '-', 'b',
                ],
            ),
        ];

        let data = parse(raw_data);
        for (part, expected) in data.chunks(OnDiskDirEntry::LEN).zip(results.iter()) {
            let on_disk_entry = OnDiskDirEntry::new(part);
            match expected {
                Expected::Lfn(start, index, contents) if on_disk_entry.is_lfn() => {
                    let (calc_start, calc_index, calc_contents) =
                        on_disk_entry.lfn_contents().unwrap();
                    assert_eq!(*start, calc_start);
                    assert_eq!(*index, calc_index);
                    assert_eq!(*contents, calc_contents);
                }
                Expected::Short(expected_entry) if !on_disk_entry.is_lfn() => {
                    let parsed_entry = on_disk_entry.get_entry(FatType::Fat32, BlockIdx(0), 0);
                    assert_eq!(*expected_entry, parsed_entry);
                }
                _ => {
                    panic!(
                        "Bad dir entry, expected:\n{:#?}\nhad\n{:#?}",
                        expected, on_disk_entry
                    );
                }
            }
        }
    }

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
        let bpb = Bpb::create_from_bytes(&BPB_EXAMPLE).unwrap();
        assert_eq!(bpb.footer(), Bpb::FOOTER_VALUE);
        assert_eq!(bpb.oem_name(), b"mkfs.fat");
        assert_eq!(bpb.bytes_per_block(), 512);
        assert_eq!(bpb.blocks_per_cluster(), 16);
        assert_eq!(bpb.reserved_block_count(), 1);
        assert_eq!(bpb.num_fats(), 2);
        assert_eq!(bpb.root_entries_count(), 512);
        assert_eq!(bpb.total_blocks16(), 0);
        assert!((bpb.media() & 0xF0) == 0xF0);
        assert_eq!(bpb.fat_size16(), 32);
        assert_eq!(bpb.blocks_per_track(), 63);
        assert_eq!(bpb.num_heads(), 255);
        assert_eq!(bpb.hidden_blocks(), 0);
        assert_eq!(bpb.total_blocks32(), 122880);
        assert_eq!(bpb.footer(), 0xAA55);
        assert_eq!(bpb.drive_number(), 0x80);
        assert_eq!(bpb.boot_signature(), 0x29);
        assert_eq!(bpb.volume_id(), 0x7771B0BB);
        assert_eq!(bpb.volume_label(), b"boot       ");
        assert_eq!(bpb.fs_type(), b"FAT16   ");
        assert_eq!(bpb.fat_size(), 32);
        assert_eq!(bpb.total_blocks(), 122880);
        assert_eq!(bpb.fat_type, FatType::Fat16);
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
