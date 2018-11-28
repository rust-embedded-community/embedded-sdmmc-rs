//! embedded-sdmmc-rs - Generic File System
//!
//! Implements generic file system components. These should be applicable to
//! most (if not all) supported filesystems.

// ****************************************************************************
//
// Imports
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Public Types
//
// ****************************************************************************

/// Things that impl this can tell you the current time.
pub trait TimeSource {
    /// Returns the current time
    fn get_timestamp(&self) -> Timestamp;
}

/// Represents a cluster on disk.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Cluster(pub(crate) u32);

/// Represents a directory entry, which tells you about
/// other files and directories.
#[derive(Debug)]
pub struct DirEntry {
    /// The name of the file
    pub name: ShortFileName,
    /// When the file was last modified
    pub mtime: Timestamp,
    /// When the file was first created
    pub ctime: Timestamp,
    /// The file attributes (Read Only, Archive, etc)
    pub attributes: Attributes,
    /// The starting cluster of the file. The FAT tells us the following Clusters.
    pub cluster: Cluster,
    /// The size of the file in bytes.
    pub size: u32,
}

/// An MS-DOS 8.3 filename. 7-bit ASCII only. All lower-case is converted to
/// upper-case.
#[derive(PartialEq, Eq)]
pub struct ShortFileName {
    pub(crate) contents: [u8; 11],
}

/// Represents an instant in time, in the local time zone. TODO: Consider
/// replacing this with POSIX time as a `u32`, which would save two bytes at
/// the expense of some maths.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct Timestamp {
    /// Add 1970 to this file to get the calendar year
    pub year_since_1970: u8,
    /// Add one to this value to get the calendar month
    pub zero_indexed_month: u8,
    /// Add one to this value to get the calendar day
    pub zero_indexed_day: u8,
    /// The number of hours past midnight
    pub hours: u8,
    /// The number of minutes past the hour
    pub minutes: u8,
    /// The number of seconds past the minute
    pub seconds: u8,
}

/// Indicates whether a directory entry is read-only, a directory, a volume
/// label, etc.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct Attributes(u8);

/// Represents an open file on disk.
pub struct File {
    pub(crate) cluster: Cluster,
    /// We only support files up to 4 GiB(!)
    pub(crate) current_offset: u32,
    pub(crate) current_length: u32,
}

/// Represents an open directory on disk.
pub struct Directory {
    pub(crate) cluster: Cluster,
}

/// The different ways we can open a file.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Mode {
    /// Open a file for reading, if it exists.
    ReadOnly,
    /// Open a file for appending (writing to the end of the existing file), if it exists.
    ReadWriteAppend,
    /// Open a file and remove all contents, before writing to the start of the existing file, if it exists.
    ReadWriteTruncate,
    /// Create a new empty file. Fail if it exists.
    ReadWriteCreate,
    /// Create a new empty file, or truncate an existing file.
    ReadWriteCreateOrTruncate,
    /// Create a new empty file, or append to an existing file.
    ReadWriteCreateOrAppend,
}

/// Various filename related errors that can occur.
#[derive(Debug, Clone)]
pub enum FilenameError {
    InvalidCharacter,
    FilenameEmpty,
    NameTooLong,
    MisplacedPeriod,
}

// ****************************************************************************
//
// Public Data
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Private Types
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Private Data
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Public Functions / Impl for Public Types
//
// ****************************************************************************

impl Cluster {
    pub const INVALID: Cluster = Cluster(0xFFFFFFFF);
    pub const BAD: Cluster = Cluster(0xFFFFFFFE);
    pub const EMPTY: Cluster = Cluster(0xFFFFFFFD);
    pub const ROOT_DIR: Cluster = Cluster(0xFFFFFFFC);
}

// impl DirEntry

impl ShortFileName {
    const FILENAME_BASE_MAX_LEN: usize = 8;
    const FILENAME_EXT_MAX_LEN: usize = 3;
    const FILENAME_MAX_LEN: usize = 11;

    /// Create a new MS-DOS 8.3 space-padded file name as stored in the directory entry.
    pub fn new(name: &str) -> Result<ShortFileName, FilenameError> {
        let mut sfn = ShortFileName {
            contents: [b' '; Self::FILENAME_MAX_LEN],
        };
        let mut idx = 0;
        let mut seen_dot = false;
        for ch in name.bytes() {
            match ch {
                // Microsoft say these are the invalid characters
                0x00...0x1F
                | 0x20
                | 0x22
                | 0x2A
                | 0x2B
                | 0x2C
                | 0x2F
                | 0x3A
                | 0x3B
                | 0x3C
                | 0x3D
                | 0x3E
                | 0x3F
                | 0x5B
                | 0x5C
                | 0x5D
                | 0x7C => {
                    return Err(FilenameError::InvalidCharacter);
                }
                // Denotes the start of the file extension
                b'.' => {
                    if idx >= 1 && idx <= Self::FILENAME_BASE_MAX_LEN {
                        idx = Self::FILENAME_BASE_MAX_LEN;
                        seen_dot = true;
                    } else {
                        return Err(FilenameError::MisplacedPeriod);
                    }
                }
                _ => {
                    let ch = if ch >= b'a' && ch <= b'z' {
                        // Uppercase characters only
                        ch - 32
                    } else {
                        ch
                    };
                    if seen_dot {
                        if idx >= Self::FILENAME_BASE_MAX_LEN && idx < Self::FILENAME_MAX_LEN {
                            sfn.contents[idx] = ch;
                        } else {
                            return Err(FilenameError::NameTooLong);
                        }
                    } else {
                        if idx < Self::FILENAME_BASE_MAX_LEN {
                            sfn.contents[idx] = ch;
                        } else {
                            return Err(FilenameError::NameTooLong);
                        }
                    }
                    idx += 1;
                }
            }
        }
        if idx == 0 {
            return Err(FilenameError::FilenameEmpty);
        }
        Ok(sfn)
    }
}

impl core::fmt::Display for ShortFileName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut printed = 0;
        for (i, &c) in self.contents.iter().enumerate() {
            if c != b' ' {
                if i == Self::FILENAME_BASE_MAX_LEN {
                    write!(f, ".")?;
                    printed += 1;
                }
                write!(f, "{}", c as char)?;
                printed += 1;
            }
        }
        if let Some(mut width) = f.width() {
            if width > printed {
                width -= printed;
                for _ in 0..width {
                    write!(f, "{}", f.fill())?;
                }
            }
        }
        Ok(())
    }
}

impl core::fmt::Debug for ShortFileName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "ShortFileName(\"{}\")", self)
    }
}

impl Timestamp {
    const MONTH_LOOKUP: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];

    /// Create a `Timestamp` from the 16-bit FAT date and time fields.
    pub fn from_fat(date: u16, time: u16) -> Timestamp {
        let year = (1980 + (date >> 9)) as u16;
        let month = ((date >> 5) & 0x000F) as u8;
        let day = ((date >> 0) & 0x001F) as u8;
        let hours = ((time >> 11) & 0x001F) as u8;
        let minutes = ((time >> 5) & 0x0003F) as u8;
        let seconds = ((time << 1) & 0x0003F) as u8;
        // Volume labels have a zero for month/day, so tolerate that...
        Timestamp {
            year_since_1970: (year - 1970) as u8,
            zero_indexed_month: if month == 0 { 0 } else { month - 1 },
            zero_indexed_day: if day == 0 { 0 } else { day - 1 },
            hours,
            minutes,
            seconds,
        }
    }
}

impl core::fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

impl core::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "{}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.year_since_1970 as u16 + 1970,
            self.zero_indexed_month + 1,
            self.zero_indexed_day + 1,
            self.hours,
            self.minutes,
            self.seconds
        )
    }
}

impl Attributes {
    pub const READ_ONLY: u8 = 0x01;
    pub const HIDDEN: u8 = 0x02;
    pub const SYSTEM: u8 = 0x04;
    pub const VOLUME: u8 = 0x08;
    pub const DIRECTORY: u8 = 0x10;
    pub const ARCHIVE: u8 = 0x20;
    pub const LFN: u8 = Self::READ_ONLY | Self::HIDDEN | Self::SYSTEM | Self::VOLUME;

    pub(crate) fn create_from_fat(value: u8) -> Attributes {
        Attributes(value)
    }

    pub fn is_read_only(&self) -> bool {
        (self.0 & Self::READ_ONLY) == Self::READ_ONLY
    }

    pub fn is_hidden(&self) -> bool {
        (self.0 & Self::HIDDEN) == Self::HIDDEN
    }

    pub fn is_system(&self) -> bool {
        (self.0 & Self::SYSTEM) == Self::SYSTEM
    }

    pub fn is_volume(&self) -> bool {
        (self.0 & Self::VOLUME) == Self::VOLUME
    }

    pub fn is_directory(&self) -> bool {
        (self.0 & Self::DIRECTORY) == Self::DIRECTORY
    }

    pub fn is_archive(&self) -> bool {
        (self.0 & Self::ARCHIVE) == Self::ARCHIVE
    }

    pub fn is_lfn(&self) -> bool {
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

impl File {}

impl Directory {}

impl FilenameError {}

// ****************************************************************************
//
// Private Functions / Impl for Priate Types
//
// ****************************************************************************

// None

// ****************************************************************************
//
// Unit Tests
//
// ****************************************************************************

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn filename_no_extension() {
        let sfn = ShortFileName {
            contents: *b"HELLO      ",
        };
        assert_eq!(format!("{}", &sfn), "HELLO");
        assert_eq!(sfn, ShortFileName::new("HELLO").unwrap());
        assert_eq!(sfn, ShortFileName::new("hello").unwrap());
        assert_eq!(sfn, ShortFileName::new("HeLlO").unwrap());
        assert_eq!(sfn, ShortFileName::new("HELLO.").unwrap());
    }

    #[test]
    fn filename_extension() {
        let sfn = ShortFileName {
            contents: *b"HELLO   TXT",
        };
        assert_eq!(format!("{}", &sfn), "HELLO.TXT");
        assert_eq!(sfn, ShortFileName::new("HELLO.TXT").unwrap());
    }

    #[test]
    fn filename_fulllength() {
        let sfn = ShortFileName {
            contents: *b"12345678TXT",
        };
        assert_eq!(format!("{}", &sfn), "12345678.TXT");
        assert_eq!(sfn, ShortFileName::new("12345678.TXT").unwrap());
    }

    #[test]
    fn filename_short_extension() {
        let sfn = ShortFileName {
            contents: *b"12345678C  ",
        };
        assert_eq!(format!("{}", &sfn), "12345678.C");
        assert_eq!(sfn, ShortFileName::new("12345678.C").unwrap());
    }

    #[test]
    fn filename_short() {
        let sfn = ShortFileName {
            contents: *b"1       C  ",
        };
        assert_eq!(format!("{}", &sfn), "1.C");
        assert_eq!(sfn, ShortFileName::new("1.C").unwrap());
    }

    #[test]
    fn filename_bad() {
        assert!(ShortFileName::new("").is_err());
        assert!(ShortFileName::new(" ").is_err());
        assert!(ShortFileName::new("123456789").is_err());
        assert!(ShortFileName::new("12345678.ABCD").is_err());
    }

}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
