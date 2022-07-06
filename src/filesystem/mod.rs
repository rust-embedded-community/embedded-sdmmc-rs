//! embedded-sdmmc-rs - Generic File System structures
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
mod timestamp;

pub use self::attributes::Attributes;
pub use self::cluster::Cluster;
pub use self::directory::{DirEntry, Directory};
pub use self::filename::{FilenameError, ShortFileName};
pub use self::files::{File, FileError, Mode};
pub use self::timestamp::{TimeSource, Timestamp};
