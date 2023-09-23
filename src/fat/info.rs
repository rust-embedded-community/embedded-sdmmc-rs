use crate::{BlockCount, BlockIdx, ClusterId};
use byteorder::{ByteOrder, LittleEndian};

/// Indentifies the supported types of FAT format
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FatSpecificInfo {
    /// Fat16 Format
    Fat16(Fat16Info),
    /// Fat32 Format
    Fat32(Fat32Info),
}

/// FAT32 specific data
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Fat32Info {
    /// The root directory does not have a reserved area in FAT32. This is the
    /// cluster it starts in (nominally 2).
    pub(crate) first_root_dir_cluster: ClusterId,
    /// Block idx of the info sector
    pub(crate) info_location: BlockIdx,
}

/// FAT16 specific data
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Fat16Info {
    /// The block the root directory starts in. Relative to start of partition
    /// (so add `self.lba_offset` before passing to volume manager)
    pub(crate) first_root_dir_block: BlockCount,
    /// Number of entries in root directory (it's reserved and not in the FAT)
    pub(crate) root_entries_count: u16,
}

/// File System Information structure is only present on FAT32 partitions. It
/// may contain a valid number of free clusters and the number of the next
/// free cluster. The information contained in the structure must be
/// considered as advisory only. File system driver implementations are not
/// required to ensure that information within the structure is kept
/// consistent.
pub struct InfoSector<'a> {
    data: &'a [u8; 512],
}

impl<'a> InfoSector<'a> {
    const LEAD_SIG: u32 = 0x4161_5252;
    const STRUC_SIG: u32 = 0x6141_7272;
    const TRAIL_SIG: u32 = 0xAA55_0000;

    /// Try and create a new Info Sector from a block.
    pub fn create_from_bytes(data: &[u8; 512]) -> Result<InfoSector, &'static str> {
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

    /// Return how many free clusters are left in this volume, if known.
    pub fn free_clusters_count(&self) -> Option<u32> {
        match self.free_count() {
            0xFFFF_FFFF => None,
            n => Some(n),
        }
    }

    /// Return the number of the next free cluster, if known.
    pub fn next_free_cluster(&self) -> Option<ClusterId> {
        match self.next_free() {
            // 0 and 1 are reserved clusters
            0xFFFF_FFFF | 0 | 1 => None,
            n => Some(ClusterId(n)),
        }
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
