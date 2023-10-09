//! Directory Entry as stored on-disk

use crate::{fat::FatType, Attributes, BlockIdx, ClusterId, DirEntry, ShortFileName, Timestamp};
use byteorder::{ByteOrder, LittleEndian};

/// Represents a 32-byte directory entry as stored on-disk in a directory file.
pub struct OnDiskDirEntry<'a> {
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

    define_field!(raw_attr, u8, 11);
    define_field!(create_time, u16, 14);
    define_field!(create_date, u16, 16);
    define_field!(last_access_data, u16, 18);
    define_field!(first_cluster_hi, u16, 20);
    define_field!(write_time, u16, 22);
    define_field!(write_date, u16, 24);
    define_field!(first_cluster_lo, u16, 26);
    define_field!(file_size, u32, 28);

    /// Create a new on-disk directory entry from a block of 32 bytes read
    /// from a directory file.
    pub fn new(data: &[u8]) -> OnDiskDirEntry {
        OnDiskDirEntry { data }
    }

    /// Is this the last entry in the directory?
    pub fn is_end(&self) -> bool {
        self.data[0] == 0x00
    }

    /// Is this a valid entry?
    pub fn is_valid(&self) -> bool {
        !self.is_end() && (self.data[0] != 0xE5)
    }

    /// Is this a Long Filename entry?
    pub fn is_lfn(&self) -> bool {
        let attributes = Attributes::create_from_fat(self.raw_attr());
        attributes.is_lfn()
    }

    /// If this is an LFN, get the contents so we can re-assemble the filename.
    pub fn lfn_contents(&self) -> Option<(bool, u8, [char; 13])> {
        if self.is_lfn() {
            let mut buffer = [' '; 13];
            let is_start = (self.data[0] & 0x40) != 0;
            let sequence = self.data[0] & 0x1F;
            // LFNs store UCS-2, so we can map from 16-bit char to 32-bit char without problem.
            buffer[0] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[1..=2]))).unwrap();
            buffer[1] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[3..=4]))).unwrap();
            buffer[2] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[5..=6]))).unwrap();
            buffer[3] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[7..=8]))).unwrap();
            buffer[4] = core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[9..=10])))
                .unwrap();
            buffer[5] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[14..=15])))
                    .unwrap();
            buffer[6] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[16..=17])))
                    .unwrap();
            buffer[7] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[18..=19])))
                    .unwrap();
            buffer[8] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[20..=21])))
                    .unwrap();
            buffer[9] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[22..=23])))
                    .unwrap();
            buffer[10] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[24..=25])))
                    .unwrap();
            buffer[11] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[28..=29])))
                    .unwrap();
            buffer[12] =
                core::char::from_u32(u32::from(LittleEndian::read_u16(&self.data[30..=31])))
                    .unwrap();
            Some((is_start, sequence, buffer))
        } else {
            None
        }
    }

    /// Does this on-disk entry match the given filename?
    pub fn matches(&self, sfn: &ShortFileName) -> bool {
        self.data[0..11] == sfn.contents
    }

    /// Which cluster, if any, does this file start at? Assumes this is from a FAT32 volume.
    pub fn first_cluster_fat32(&self) -> ClusterId {
        let cluster_no =
            (u32::from(self.first_cluster_hi()) << 16) | u32::from(self.first_cluster_lo());
        ClusterId(cluster_no)
    }

    /// Which cluster, if any, does this file start at? Assumes this is from a FAT16 volume.
    fn first_cluster_fat16(&self) -> ClusterId {
        let cluster_no = u32::from(self.first_cluster_lo());
        ClusterId(cluster_no)
    }

    /// Convert the on-disk format into a DirEntry
    pub fn get_entry(
        &self,
        fat_type: FatType,
        entry_block: BlockIdx,
        entry_offset: u32,
    ) -> DirEntry {
        let attributes = Attributes::create_from_fat(self.raw_attr());
        let mut result = DirEntry {
            name: ShortFileName {
                contents: [0u8; 11],
            },
            mtime: Timestamp::from_fat(self.write_date(), self.write_time()),
            ctime: Timestamp::from_fat(self.create_date(), self.create_time()),
            attributes,
            cluster: {
                let cluster = if fat_type == FatType::Fat32 {
                    self.first_cluster_fat32()
                } else {
                    self.first_cluster_fat16()
                };
                if cluster == ClusterId::EMPTY && attributes.is_directory() {
                    // FAT16/FAT32 uses a cluster ID of `0` in the ".." entry to mean 'root directory'
                    ClusterId::ROOT_DIR
                } else {
                    cluster
                }
            },
            size: self.file_size(),
            entry_block,
            entry_offset,
        };
        result.name.contents.copy_from_slice(&self.data[0..11]);
        result
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
