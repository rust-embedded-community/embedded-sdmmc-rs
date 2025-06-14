/// Identifies a cluster on disk.
///
/// A cluster is a consecutive group of blocks. Each cluster has a a numeric ID.
/// Some numeric IDs are reserved for special purposes.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct ClusterId(pub(crate) u32);

impl ClusterId {
    /// Magic value indicating an invalid cluster value.
    pub const INVALID: ClusterId = ClusterId(0xFFFF_FFF6);
    /// Magic value indicating a bad cluster.
    pub const BAD: ClusterId = ClusterId(0xFFFF_FFF7);
    /// Magic value indicating a empty cluster.
    pub const EMPTY: ClusterId = ClusterId(0x0000_0000);
    /// Magic value indicating the cluster holding the root directory (which
    /// doesn't have a number in FAT16 as there's a reserved region).
    pub const ROOT_DIR: ClusterId = ClusterId(0xFFFF_FFFC);
    /// Magic value indicating that the cluster is allocated and is the final cluster for the file
    pub const END_OF_FILE: ClusterId = ClusterId(0xFFFF_FFFF);
}

impl core::ops::Add<u32> for ClusterId {
    type Output = ClusterId;
    fn add(self, rhs: u32) -> ClusterId {
        ClusterId(self.0 + rhs)
    }
}

impl core::ops::AddAssign<u32> for ClusterId {
    fn add_assign(&mut self, rhs: u32) {
        self.0 += rhs;
    }
}

impl core::fmt::Debug for ClusterId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ClusterId(")?;
        match *self {
            Self::INVALID => {
                write!(f, "{:08}", "INVALID")?;
            }
            Self::BAD => {
                write!(f, "{:08}", "BAD")?;
            }
            Self::EMPTY => {
                write!(f, "{:08}", "EMPTY")?;
            }
            Self::ROOT_DIR => {
                write!(f, "{:08}", "ROOT")?;
            }
            Self::END_OF_FILE => {
                write!(f, "{:08}", "EOF")?;
            }
            ClusterId(value) => {
                write!(f, "{:08x}", value)?;
            }
        }
        write!(f, ")")?;
        Ok(())
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
