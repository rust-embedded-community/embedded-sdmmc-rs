//! embedded-sdmmc-rs - Generic File System
//!
//! Implements generic file system components

/// Things that impl this can tell you the current time.
pub trait TimeSource {
    /// Returns the current time
    fn get_timestamp(&self) -> Timestamp;
}

/// Represents a cluster on disk.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Inode(pub(crate) u32);

impl Inode {
    pub const INVALID: Inode = Inode(0xFFFFFFFF);
    pub const BAD: Inode = Inode(0xFFFFFFFE);
    pub const EMPTY: Inode = Inode(0xFFFFFFFD);
    pub const ROOT_DIR: Inode = Inode(0xFFFFFFFC);
}

/// Represents a directory on disk.
pub struct Directory {
    /// If None, this is the root directory (which is special)
    pub(crate) inode: Inode,
}

/// Represents an instant in time, in the local time zone. TODO: Consider
/// replacing this with POSIX time as a `u32`, which would save two bytes at
/// the expense of some maths.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct Timestamp {
    pub year_since_1970: u8,
    pub zero_indexed_month: u8,
    pub zero_indexed_day: u8,
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
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

#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct Attributes(u8);

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

/// An MS-DOS 8.3 filename. 7-bit ASCII only. All lower-case is converted to
/// upper-case.
#[derive(PartialEq, Eq)]
pub struct ShortFileName {
    pub(crate) contents: [u8; 11],
}

const FILENAME_LEN: usize = 8;

impl core::fmt::Display for ShortFileName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        for (i, &c) in self.contents.iter().enumerate() {
            if c != b' ' {
                if i == FILENAME_LEN {
                    write!(f, ".")?;
                }
                write!(f, "{}", c as char)?;
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

#[derive(Debug, Clone)]
pub enum FilenameError {
    InvalidCharacter,
    FilenameEmpty,
    NameTooLong,
    MisplacedPeriod,
}

impl ShortFileName {
    /// Create a new MS-DOS 8.3 space-padded file name as stored in the directory entry.
    pub fn new(name: &str) -> Result<ShortFileName, FilenameError> {
        let mut sfn = ShortFileName {
            contents: [b' '; 11],
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
                    if idx >= 1 && idx <= 8 {
                        idx = 8;
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
                        if idx >= 8 && idx <= 10 {
                            sfn.contents[idx] = ch;
                        } else {
                            return Err(FilenameError::NameTooLong);
                        }
                    } else {
                        if idx <= 7 {
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

#[derive(Debug)]
pub struct DirEntry {
    pub name: ShortFileName,
    pub mtime: Timestamp,
    pub ctime: Timestamp,
    pub attributes: Attributes,
    pub inode: Inode,
    pub size: u32,
}

pub struct File {
    pub(crate) inode: Inode,
    /// We only support files up to 4 GiB(!)
    pub(crate) current_offset: u32,
    pub(crate) current_length: u32,
}

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
