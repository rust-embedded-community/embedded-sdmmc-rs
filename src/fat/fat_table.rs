/// FAT table definition
///
///
use crate::fat::FatType;
use byteorder::{ByteOrder, LittleEndian};

/// Represents a single FAT table. It contains all information about which cluster is occupied
/// from a file
pub struct FatTable<'a> {
    fat_type: FatType,
    data: &'a mut [u8],
}

impl<'a> FatTable<'a> {
    /// Attempt to parse a FAT table from a multiple sectors.
    pub fn create_from_bytes(data: &'a mut [u8], fat_type: FatType) -> Result<Self, &'static str> {
        Ok(Self { data, fat_type })
    }

    // FAT16 only
    //define_field!(fat_id16, u16, 0);

    // FAT32 only
    //define_field!(fat_id32, u32, 0);

    const FAT16_DIRTY_BIT: u16 = 15;
    const FAT32_DIRTY_BIT: u32 = 27;

    pub(crate) fn dirty(&self) -> bool {
        match self.fat_type {
            FatType::Fat16 => {
                (LittleEndian::read_u16(&self.data[2..2 + 2]) & (1 << Self::FAT16_DIRTY_BIT)) == 0
            }
            FatType::Fat32 => {
                (LittleEndian::read_u32(&self.data[4..4 + 4]) & (1 << Self::FAT32_DIRTY_BIT)) == 0
            }
        }
    }

    pub(crate) fn set_dirty(&mut self, dirty: bool) {
        match self.fat_type {
            FatType::Fat16 => {
                let mut v = LittleEndian::read_u16(&self.data[2..2 + 2]);
                if dirty {
                    v &= !(1 << Self::FAT16_DIRTY_BIT);
                } else {
                    v |= 1 << Self::FAT16_DIRTY_BIT
                }
                LittleEndian::write_u16(&mut self.data[2..2 + 2], v);
            }
            FatType::Fat32 => {
                let mut v = LittleEndian::read_u32(&self.data[4..4 + 4]);
                if dirty {
                    v &= !(1 << Self::FAT32_DIRTY_BIT);
                } else {
                    v |= 1 << Self::FAT32_DIRTY_BIT
                }
                LittleEndian::write_u32(&mut self.data[4..4 + 4], v);
            }
        }
    }
}
