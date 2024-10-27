//! Filename related types

use crate::fat::VolumeName;
use crate::trace;

/// Various filename related errors that can occur.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub enum FilenameError {
    /// Tried to create a file with an invalid character.
    InvalidCharacter,
    /// Tried to create a file with no file name.
    FilenameEmpty,
    /// Given name was too long (we are limited to 8.3).
    NameTooLong,
    /// Can't start a file with a period, or after 8 characters.
    MisplacedPeriod,
    /// Can't extract utf8 from file name
    Utf8Error,
}

/// Describes things we can convert to short 8.3 filenames
pub trait ToShortFileName {
    /// Try and convert this value into a [`ShortFileName`].
    fn to_short_filename(self) -> Result<ShortFileName, FilenameError>;
}

impl ToShortFileName for ShortFileName {
    fn to_short_filename(self) -> Result<ShortFileName, FilenameError> {
        Ok(self)
    }
}

impl ToShortFileName for &ShortFileName {
    fn to_short_filename(self) -> Result<ShortFileName, FilenameError> {
        Ok(self.clone())
    }
}

impl ToShortFileName for &str {
    fn to_short_filename(self) -> Result<ShortFileName, FilenameError> {
        ShortFileName::create_from_str(self)
    }
}

/// An MS-DOS 8.3 filename.
///
/// ISO-8859-1 encoding is assumed. All lower-case is converted to upper-case by
/// default.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(PartialEq, Eq, Clone)]
pub struct ShortFileName {
    pub(crate) contents: [u8; Self::TOTAL_LEN],
}

impl ShortFileName {
    const BASE_LEN: usize = 8;
    const TOTAL_LEN: usize = 11;

    /// Get a short file name containing "..", which means "parent directory".
    pub const fn parent_dir() -> Self {
        Self {
            contents: *b"..         ",
        }
    }

    /// Get a short file name containing ".", which means "this directory".
    pub const fn this_dir() -> Self {
        Self {
            contents: *b".          ",
        }
    }

    /// Get base name (without extension) of the file.
    pub fn base_name(&self) -> &[u8] {
        Self::bytes_before_space(&self.contents[..Self::BASE_LEN])
    }

    /// Get extension of the file (without base name).
    pub fn extension(&self) -> &[u8] {
        Self::bytes_before_space(&self.contents[Self::BASE_LEN..])
    }

    fn bytes_before_space(bytes: &[u8]) -> &[u8] {
        bytes.split(|b| *b == b' ').next().unwrap_or(&[])
    }

    /// Create a new MS-DOS 8.3 space-padded file name as stored in the directory entry.
    ///
    /// The output uses ISO-8859-1 encoding.
    pub fn create_from_str(name: &str) -> Result<ShortFileName, FilenameError> {
        let mut sfn = ShortFileName {
            contents: [b' '; Self::TOTAL_LEN],
        };

        // Special case `..`, which means "parent directory".
        if name == ".." {
            return Ok(ShortFileName::parent_dir());
        }

        // Special case `.` (or blank), which means "this directory".
        if name.is_empty() || name == "." {
            return Ok(ShortFileName::this_dir());
        }

        let mut idx = 0;
        let mut seen_dot = false;
        for ch in name.chars() {
            match ch {
                // Microsoft say these are the invalid characters
                '\u{0000}'..='\u{001F}'
                | '"'
                | '*'
                | '+'
                | ','
                | '/'
                | ':'
                | ';'
                | '<'
                | '='
                | '>'
                | '?'
                | '['
                | '\\'
                | ']'
                | ' '
                | '|' => {
                    return Err(FilenameError::InvalidCharacter);
                }
                x if x > '\u{00FF}' => {
                    // We only handle ISO-8859-1 which is Unicode Code Points
                    // \U+0000 to \U+00FF. This is above that.
                    return Err(FilenameError::InvalidCharacter);
                }
                '.' => {
                    // Denotes the start of the file extension
                    if (1..=Self::BASE_LEN).contains(&idx) {
                        idx = Self::BASE_LEN;
                        seen_dot = true;
                    } else {
                        return Err(FilenameError::MisplacedPeriod);
                    }
                }
                _ => {
                    let b = ch.to_ascii_uppercase() as u8;
                    if seen_dot {
                        if (Self::BASE_LEN..Self::TOTAL_LEN).contains(&idx) {
                            sfn.contents[idx] = b;
                        } else {
                            return Err(FilenameError::NameTooLong);
                        }
                    } else if idx < Self::BASE_LEN {
                        sfn.contents[idx] = b;
                    } else {
                        return Err(FilenameError::NameTooLong);
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

    /// Convert a Short File Name to a Volume Label.
    ///
    /// # Safety
    ///
    /// Volume Labels can contain things that Short File Names cannot, so only
    /// do this conversion if you have the name of a directory entry with the
    /// 'Volume Label' attribute.
    pub unsafe fn to_volume_label(self) -> VolumeName {
        VolumeName {
            contents: self.contents,
        }
    }

    /// Get the LFN checksum for this short filename
    pub fn csum(&self) -> u8 {
        let mut result = 0u8;
        for b in self.contents.iter() {
            result = result.rotate_right(1).wrapping_add(*b);
        }
        result
    }
}

impl core::fmt::Display for ShortFileName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut printed = 0;
        for (i, &c) in self.contents.iter().enumerate() {
            if c != b' ' {
                if i == Self::BASE_LEN {
                    write!(f, ".")?;
                    printed += 1;
                }
                // converting a byte to a codepoint means you are assuming
                // ISO-8859-1 encoding, because that's how Unicode was designed.
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

/// Used to store a Long File Name
pub struct LfnBuffer<'a> {
    /// We fill this buffer in from the back
    inner: &'a mut [u8],
    /// How many bytes are free.
    ///
    /// This is also the byte index the string starts from.
    free: usize,
    /// Did we overflow?
    overflow: bool,
}

impl<'a> LfnBuffer<'a> {
    /// Create a new, empty, LFN Buffer using the given mutable slice as its storage.
    pub fn new(storage: &'a mut [u8]) -> LfnBuffer<'a> {
        let len = storage.len();
        LfnBuffer {
            inner: storage,
            free: len,
            overflow: false,
        }
    }

    /// Empty out this buffer
    pub fn clear(&mut self) {
        self.free = self.inner.len();
        self.overflow = false;
    }

    /// Push the 13 UCS-2 characters into this string
    ///
    /// We assume they are pushed last-chunk-first, as you would find
    /// them on disk.
    pub fn push(&mut self, buffer: &[u16; 13]) {
        // find the first null, if any
        let null_idx = buffer
            .iter()
            .position(|&b| b == 0x0000)
            .unwrap_or(buffer.len());
        // take all the wide chars, up to the null (or go to the end)
        let buffer = &buffer[0..null_idx];
        for ch in buffer.iter().rev() {
            let ch = char::from_u32(*ch as u32).unwrap_or('?');
            trace!("LFN push {:?}", ch);
            let mut ch_bytes = [0u8; 4];
            // a buffer of length 4 is always enough
            let ch_str = ch.encode_utf8(&mut ch_bytes);
            if self.free < ch_str.len() {
                self.overflow = true;
                return;
            }
            // store the encoded character in the buffer, working backwards
            for b in ch_str.bytes().rev() {
                self.free -= 1;
                self.inner[self.free] = b;
            }
        }
    }

    /// View this LFN buffer as a string-slice
    pub fn as_str(&self) -> &str {
        if self.overflow {
            ""
        } else {
            // we always only put UTF-8 encoded data in here
            unsafe { core::str::from_utf8_unchecked(&self.inner[self.free..]) }
        }
    }
}

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
        assert_eq!(sfn, ShortFileName::create_from_str("HELLO").unwrap());
        assert_eq!(sfn, ShortFileName::create_from_str("hello").unwrap());
        assert_eq!(sfn, ShortFileName::create_from_str("HeLlO").unwrap());
        assert_eq!(sfn, ShortFileName::create_from_str("HELLO.").unwrap());
    }

    #[test]
    fn filename_extension() {
        let sfn = ShortFileName {
            contents: *b"HELLO   TXT",
        };
        assert_eq!(format!("{}", &sfn), "HELLO.TXT");
        assert_eq!(sfn, ShortFileName::create_from_str("HELLO.TXT").unwrap());
    }

    #[test]
    fn filename_get_extension() {
        let mut sfn = ShortFileName::create_from_str("hello.txt").unwrap();
        assert_eq!(sfn.extension(), "TXT".as_bytes());
        sfn = ShortFileName::create_from_str("hello").unwrap();
        assert_eq!(sfn.extension(), "".as_bytes());
        sfn = ShortFileName::create_from_str("hello.a").unwrap();
        assert_eq!(sfn.extension(), "A".as_bytes());
    }

    #[test]
    fn filename_get_base_name() {
        let mut sfn = ShortFileName::create_from_str("hello.txt").unwrap();
        assert_eq!(sfn.base_name(), "HELLO".as_bytes());
        sfn = ShortFileName::create_from_str("12345678").unwrap();
        assert_eq!(sfn.base_name(), "12345678".as_bytes());
        sfn = ShortFileName::create_from_str("1").unwrap();
        assert_eq!(sfn.base_name(), "1".as_bytes());
    }

    #[test]
    fn filename_fulllength() {
        let sfn = ShortFileName {
            contents: *b"12345678TXT",
        };
        assert_eq!(format!("{}", &sfn), "12345678.TXT");
        assert_eq!(sfn, ShortFileName::create_from_str("12345678.TXT").unwrap());
    }

    #[test]
    fn filename_short_extension() {
        let sfn = ShortFileName {
            contents: *b"12345678C  ",
        };
        assert_eq!(format!("{}", &sfn), "12345678.C");
        assert_eq!(sfn, ShortFileName::create_from_str("12345678.C").unwrap());
    }

    #[test]
    fn filename_short() {
        let sfn = ShortFileName {
            contents: *b"1       C  ",
        };
        assert_eq!(format!("{}", &sfn), "1.C");
        assert_eq!(sfn, ShortFileName::create_from_str("1.C").unwrap());
    }

    #[test]
    fn filename_empty() {
        assert_eq!(
            ShortFileName::create_from_str("").unwrap(),
            ShortFileName::this_dir()
        );
    }

    #[test]
    fn filename_bad() {
        assert!(ShortFileName::create_from_str(" ").is_err());
        assert!(ShortFileName::create_from_str("123456789").is_err());
        assert!(ShortFileName::create_from_str("12345678.ABCD").is_err());
    }

    #[test]
    fn checksum() {
        assert_eq!(
            0xB3,
            ShortFileName::create_from_str("UNARCH~1.DAT")
                .unwrap()
                .csum()
        );
    }

    #[test]
    fn one_piece() {
        let mut storage = [0u8; 64];
        let mut buf: LfnBuffer = LfnBuffer::new(&mut storage);
        buf.push(&[
            0x0030, 0x0031, 0x0032, 0x0033, 0x2202, 0x0000, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF,
            0xFFFF, 0xFFFF,
        ]);
        assert_eq!(buf.as_str(), "0123∂");
    }

    #[test]
    fn two_piece() {
        let mut storage = [0u8; 64];
        let mut buf: LfnBuffer = LfnBuffer::new(&mut storage);
        buf.push(&[
            0x0030, 0x0031, 0x0032, 0x0033, 0x2202, 0x0000, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF,
            0xFFFF, 0xFFFF,
        ]);
        buf.push(&[
            0x0041, 0x0042, 0x0043, 0x0044, 0x0045, 0x0046, 0x0047, 0x0048, 0x0049, 0x004a, 0x004b,
            0x004c, 0x004d,
        ]);
        assert_eq!(buf.as_str(), "ABCDEFGHIJKLM0123∂");
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
