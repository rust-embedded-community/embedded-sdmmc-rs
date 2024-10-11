//! Generic File System structures
//!
//! Implements generic file system components. These should be applicable to
//! most (if not all) supported filesystems.

/// Maximum file size supported by this library
pub const MAX_FILE_SIZE: u32 = core::u32::MAX;

mod attributes;
mod cluster;
mod directory;
mod filename;
mod files;
mod handles;
mod timestamp;

pub use self::attributes::Attributes;
pub use self::cluster::ClusterId;
pub use self::directory::{DirEntry, Directory, RawDirectory};
pub use self::filename::{FilenameError, ShortFileName, ToShortFileName};
pub use self::files::{File, FileError, Mode, RawFile};
pub use self::handles::{Handle, HandleGenerator};
pub use self::timestamp::{TimeSource, Timestamp};

pub(crate) use self::directory::DirectoryInfo;
pub(crate) use self::files::FileInfo;

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
