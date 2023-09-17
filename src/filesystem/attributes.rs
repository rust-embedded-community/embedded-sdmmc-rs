/// Indicates whether a directory entry is read-only, a directory, a volume
/// label, etc.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct Attributes(pub(crate) u8);

impl Attributes {
    /// Indicates this file cannot be written.
    pub const READ_ONLY: u8 = 0x01;
    /// Indicates the file is hidden.
    pub const HIDDEN: u8 = 0x02;
    /// Indicates this is a system file.
    pub const SYSTEM: u8 = 0x04;
    /// Indicates this is a volume label.
    pub const VOLUME: u8 = 0x08;
    /// Indicates this is a directory.
    pub const DIRECTORY: u8 = 0x10;
    /// Indicates this file needs archiving (i.e. has been modified since last
    /// archived).
    pub const ARCHIVE: u8 = 0x20;
    /// This set of flags indicates the file is actually a long file name
    /// fragment.
    pub const LFN: u8 = Self::READ_ONLY | Self::HIDDEN | Self::SYSTEM | Self::VOLUME;

    /// Create a `Attributes` value from the `u8` stored in a FAT16/FAT32
    /// Directory Entry.
    pub(crate) fn create_from_fat(value: u8) -> Attributes {
        Attributes(value)
    }

    pub(crate) fn set_archive(&mut self, flag: bool) {
        let archive = if flag { 0x20 } else { 0x00 };
        self.0 |= archive;
    }

    /// Does this file has the read-only attribute set?
    pub fn is_read_only(self) -> bool {
        (self.0 & Self::READ_ONLY) == Self::READ_ONLY
    }

    /// Does this file has the hidden attribute set?
    pub fn is_hidden(self) -> bool {
        (self.0 & Self::HIDDEN) == Self::HIDDEN
    }

    /// Does this file has the system attribute set?
    pub fn is_system(self) -> bool {
        (self.0 & Self::SYSTEM) == Self::SYSTEM
    }

    /// Does this file has the volume attribute set?
    pub fn is_volume(self) -> bool {
        (self.0 & Self::VOLUME) == Self::VOLUME
    }

    /// Does this entry point at a directory?
    pub fn is_directory(self) -> bool {
        (self.0 & Self::DIRECTORY) == Self::DIRECTORY
    }

    /// Does this need archiving?
    pub fn is_archive(self) -> bool {
        (self.0 & Self::ARCHIVE) == Self::ARCHIVE
    }

    /// Is this a long file name fragment?
    pub fn is_lfn(self) -> bool {
        (self.0 & Self::LFN) == Self::LFN
    }
}

impl core::fmt::Debug for Attributes {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.is_lfn() {
            write!(f, "LFN")?;
        } else {
            if self.is_directory() {
                write!(f, "D")?;
            } else {
                write!(f, "F")?;
            }
            if self.is_read_only() {
                write!(f, "R")?;
            }
            if self.is_hidden() {
                write!(f, "H")?;
            }
            if self.is_system() {
                write!(f, "S")?;
            }
            if self.is_volume() {
                write!(f, "V")?;
            }
            if self.is_archive() {
                write!(f, "A")?;
            }
        }
        Ok(())
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
