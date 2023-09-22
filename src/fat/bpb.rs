//! Boot Parameter Block

use crate::{
    blockdevice::BlockCount,
    fat::{FatType, OnDiskDirEntry},
};
use byteorder::{ByteOrder, LittleEndian};

/// Represents a Boot Parameter Block. This is the first sector of a FAT
/// formatted partition, and it describes various properties of the FAT
/// filesystem.
pub struct Bpb<'a> {
    data: &'a [u8; 512],
    pub(crate) fat_type: FatType,
    cluster_count: u32,
}

impl<'a> Bpb<'a> {
    pub(crate) const FOOTER_VALUE: u16 = 0xAA55;

    /// Attempt to parse a Boot Parameter Block from a 512 byte sector.
    pub fn create_from_bytes(data: &[u8; 512]) -> Result<Bpb, &'static str> {
        let mut bpb = Bpb {
            data,
            fat_type: FatType::Fat16,
            cluster_count: 0,
        };
        if bpb.footer() != Self::FOOTER_VALUE {
            return Err("Bad BPB footer");
        }

        let root_dir_blocks =
            BlockCount::from_bytes(u32::from(bpb.root_entries_count()) * OnDiskDirEntry::LEN_U32).0;
        let non_data_blocks = u32::from(bpb.reserved_block_count())
            + (u32::from(bpb.num_fats()) * bpb.fat_size())
            + root_dir_blocks;
        let data_blocks = bpb.total_blocks() - non_data_blocks;
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

    /// Get the OEM name string for this volume
    pub fn oem_name(&self) -> &[u8] {
        &self.data[3..11]
    }

    // FAT16/FAT32 functions

    /// Get the Volume Label string for this volume
    pub fn volume_label(&self) -> &[u8] {
        if self.fat_type != FatType::Fat32 {
            &self.data[43..=53]
        } else {
            &self.data[71..=81]
        }
    }

    // FAT32 only functions

    /// On a FAT32 volume, return the free block count from the Info Block. On
    /// a FAT16 volume, returns None.
    pub fn fs_info_block(&self) -> Option<BlockCount> {
        if self.fat_type != FatType::Fat32 {
            None
        } else {
            Some(BlockCount(u32::from(self.fs_info())))
        }
    }

    // Magic functions that get the right FAT16/FAT32 result

    /// Get the size of the File Allocation Table in blocks.
    pub fn fat_size(&self) -> u32 {
        let result = u32::from(self.fat_size16());
        if result != 0 {
            result
        } else {
            self.fat_size32()
        }
    }

    /// Get the total number of blocks in this filesystem.
    pub fn total_blocks(&self) -> u32 {
        let result = u32::from(self.total_blocks16());
        if result != 0 {
            result
        } else {
            self.total_blocks32()
        }
    }

    /// Get the total number of clusters in this filesystem.
    pub fn total_clusters(&self) -> u32 {
        self.cluster_count
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
