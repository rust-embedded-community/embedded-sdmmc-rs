//! # embedded-sdmmc
//!
//! > An SD/MMC Library written in Embedded Rust
//!
//! This crate is intended to allow you to read/write files on a FAT formatted
//! SD card on your Rust Embedded device, as easily as using the `SdFat` Arduino
//! library. It is written in pure-Rust, is `#![no_std]` and does not use
//! `alloc` or `collections` to keep the memory footprint low. In the first
//! instance it is designed for readability and simplicity over performance.
//!
//! ## Using the crate
//!
//! You will need something that implements the `BlockDevice` trait, which can
//! read and write the 512-byte blocks (or sectors) from your card. If you were
//! to implement this over USB Mass Storage, there's no reason this crate
//! couldn't work with a USB Thumb Drive, but we only supply a `BlockDevice`
//! suitable for reading SD and SDHC cards over SPI.
//!
//! ```rust
//! use embedded_sdmmc::{Error, Mode, SdCard, SdCardError, TimeSource, VolumeIdx, VolumeManager};
//!
//! fn example<S, D, T>(spi: S, delay: D, ts: T) -> Result<(), Error<SdCardError>>
//! where
//!     S: embedded_hal::spi::SpiDevice,
//!     D: embedded_hal::delay::DelayNs,
//!     T: TimeSource,
//! {
//!     let sdcard = SdCard::new(spi, delay);
//!     println!("Card size is {} bytes", sdcard.num_bytes()?);
//!     let volume_mgr = VolumeManager::new(sdcard, ts);
//!     let volume0 = volume_mgr.open_volume(VolumeIdx(0))?;
//!     println!("Volume 0: {:?}", volume0);
//!     let root_dir = volume0.open_root_dir()?;
//!     let mut my_file = root_dir.open_file_in_dir("MY_FILE.TXT", Mode::ReadOnly)?;
//!     while !my_file.is_eof() {
//!         let mut buffer = [0u8; 32];
//!         let num_read = my_file.read(&mut buffer)?;
//!         for b in &buffer[0..num_read] {
//!             print!("{}", *b as char);
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! For writing files:
//!
//! ```rust
//! use embedded_sdmmc::{BlockDevice, Directory, Error, Mode, TimeSource};
//! fn write_file<D: BlockDevice, T: TimeSource, const DIRS: usize, const FILES: usize, const VOLUMES: usize>(
//!     root_dir: &mut Directory<D, T, DIRS, FILES, VOLUMES>,
//! ) -> Result<(), Error<D::Error>>
//! {
//!     let my_other_file = root_dir.open_file_in_dir("MY_DATA.CSV", Mode::ReadWriteCreateOrAppend)?;
//!     my_other_file.write(b"Timestamp,Signal,Value\n")?;
//!     my_other_file.write(b"2025-01-01T00:00:00Z,TEMP,25.0\n")?;
//!     my_other_file.write(b"2025-01-01T00:00:01Z,TEMP,25.1\n")?;
//!     my_other_file.write(b"2025-01-01T00:00:02Z,TEMP,25.2\n")?;
//!     // Don't forget to flush the file so that the directory entry is updated
//!     my_other_file.flush()?;
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! * `log`: Enabled by default. Generates log messages using the `log` crate.
//! * `defmt-log`: By turning off the default features and enabling the
//!   `defmt-log` feature you can configure this crate to log messages over defmt
//!   instead.
//!
//! You cannot enable both the `log` feature and the `defmt-log` feature.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

// ****************************************************************************
//
// Imports
//
// ****************************************************************************
#[macro_use]
mod structure;

/// Re-export the tyoes library.
pub use embedded_sdmmc_types;
pub use embedded_sdmmc_types::blockdevice;
pub mod fat;
pub mod filesystem;
pub mod sdcard;

use core::fmt::Debug;
use embedded_io::ErrorKind;
use filesystem::Handle;

#[doc(inline)]
pub use crate::blockdevice::{Block, BlockCount, BlockDevice, BlockIdx};

#[doc(inline)]
pub use crate::fat::{FatVolume, VolumeName};

#[doc(inline)]
pub use crate::filesystem::{
    Attributes, ClusterId, DirEntry, Directory, File, FilenameError, LfnBuffer, MAX_FILE_SIZE,
    Mode, RawDirectory, RawFile, ShortFileName, TimeSource, Timestamp,
};

use filesystem::DirectoryInfo;

#[doc(inline)]
pub use crate::sdcard::spi::Error as SdCardError;

#[doc(inline)]
pub use crate::sdcard::spi::SdCard;

mod volume_mgr;
#[doc(inline)]
pub use volume_mgr::VolumeManager;

#[cfg(all(feature = "defmt-log", feature = "log"))]
compile_error!("Cannot enable both log and defmt-log");

#[cfg(feature = "log")]
use log::{debug, trace, warn};

#[cfg(feature = "defmt-log")]
use defmt::{debug, trace, warn};

#[cfg(all(not(feature = "defmt-log"), not(feature = "log")))]
#[macro_export]
/// Like log::debug! but does nothing at all
macro_rules! debug {
    ($($arg:tt)+) => {};
}

#[cfg(all(not(feature = "defmt-log"), not(feature = "log")))]
#[macro_export]
/// Like log::trace! but does nothing at all
macro_rules! trace {
    ($($arg:tt)+) => {};
}

#[cfg(all(not(feature = "defmt-log"), not(feature = "log")))]
#[macro_export]
/// Like log::warn! but does nothing at all
macro_rules! warn {
    ($($arg:tt)+) => {};
}

// ****************************************************************************
//
// Public Types
//
// ****************************************************************************

/// All the ways the functions in this crate can fail.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error<E>
where
    E: core::error::Error,
{
    /// The underlying block device threw an error.
    #[error("error from underlying block device: {0}")]
    DeviceError(#[from] E),
    /// The filesystem is badly formatted (or this code is buggy).
    #[error("filesystem is badly formatted: {0}")]
    FormatError(&'static str),
    /// The given `VolumeIdx` was bad,
    #[error("no such volume")]
    NoSuchVolume,
    /// The given filename was bad
    #[error("bad filename")]
    FilenameError(FilenameError),
    /// Out of memory opening volumes
    #[error("too many open volumes")]
    TooManyOpenVolumes,
    /// Out of memory opening directories
    #[error("too many open directories")]
    TooManyOpenDirs,
    /// Out of memory opening files
    #[error("too many open files")]
    TooManyOpenFiles,
    /// Bad handle given
    #[error("bad handle")]
    BadHandle,
    /// That file or directory doesn't exist
    #[error("file or directory does not exist")]
    NotFound,
    /// You can't open a file twice or delete an open file
    #[error("file already open")]
    FileAlreadyOpen,
    /// You can't open a directory twice
    #[error("directory already open")]
    DirAlreadyOpen,
    /// You can't open a directory as a file
    #[error("cannot open directory as file")]
    OpenedDirAsFile,
    /// You can't open a file as a directory
    #[error("cannot open file as directory")]
    OpenedFileAsDir,
    /// You can't delete a non-empty directory
    #[error("cannot delete a non-empty directory")]
    DeleteNonEmptyDir,
    /// You can't close a volume with open files or directories
    #[error("volume is still in use")]
    VolumeStillInUse,
    /// You can't open a volume twice
    #[error("cannot open volume twice")]
    VolumeAlreadyOpen,
    /// We can't do that yet
    #[error("unsupported operation")]
    Unsupported,
    /// Tried to read beyond end of file
    #[error("end of file")]
    EndOfFile,
    /// Found a bad cluster
    #[error("bad cluster")]
    BadCluster,
    /// Error while converting types
    #[error("type conversion failed")]
    ConversionError,
    /// The device does not have enough space for the operation
    #[error("not enough space on device")]
    NotEnoughSpace,
    /// Cluster was not properly allocated by the library
    #[error("cluster not properly allocated")]
    AllocationError,
    /// Jumped to free space during FAT traversing
    #[error("FAT chain unterminated")]
    UnterminatedFatChain,
    /// Tried to open Read-Only file with write mode
    #[error("file is read-only")]
    ReadOnly,
    /// Tried to create an existing file
    #[error("file already exists")]
    FileAlreadyExists,
    /// Bad block size - only 512 byte blocks supported
    #[error("bad block size: {0} (only 512 byte blocks supported)")]
    BadBlockSize(u16),
    /// Bad offset given when seeking
    #[error("invalid seek offset")]
    InvalidOffset,
    /// Disk is full
    #[error("disk full")]
    DiskFull,
    /// A directory with that name already exists
    #[error("directory already exists")]
    DirAlreadyExists,
    /// The filesystem tried to gain a lock whilst already locked.
    ///
    /// This is either a bug in the filesystem, or you tried to access the
    /// filesystem API from inside a directory iterator (that isn't allowed).
    #[error("already locked")]
    LockError,
}

impl<E: core::error::Error + 'static> embedded_io::Error for Error<E> {
    fn kind(&self) -> ErrorKind {
        match self {
            Error::DeviceError(_)
            | Error::FormatError(_)
            | Error::FileAlreadyOpen
            | Error::DirAlreadyOpen
            | Error::VolumeStillInUse
            | Error::VolumeAlreadyOpen
            | Error::EndOfFile
            | Error::DiskFull
            | Error::NotEnoughSpace
            | Error::AllocationError
            | Error::LockError => ErrorKind::Other,
            Error::NoSuchVolume
            | Error::FilenameError(_)
            | Error::BadHandle
            | Error::InvalidOffset => ErrorKind::InvalidInput,
            Error::TooManyOpenVolumes | Error::TooManyOpenDirs | Error::TooManyOpenFiles => {
                ErrorKind::OutOfMemory
            }
            Error::NotFound => ErrorKind::NotFound,
            Error::OpenedDirAsFile
            | Error::OpenedFileAsDir
            | Error::DeleteNonEmptyDir
            | Error::BadCluster
            | Error::ConversionError
            | Error::UnterminatedFatChain => ErrorKind::InvalidData,
            Error::Unsupported | Error::BadBlockSize(_) => ErrorKind::Unsupported,
            Error::ReadOnly => ErrorKind::PermissionDenied,
            Error::FileAlreadyExists | Error::DirAlreadyExists => ErrorKind::AlreadyExists,
        }
    }
}

/// A handle to a volume.
///
/// A volume is a partition with a filesystem within it.
///
/// Do NOT drop this object! It doesn't hold a reference to the Volume Manager
/// it was created from and the VolumeManager will think you still have the
/// volume open if you just drop it, and it won't let you open the file again.
///
/// Instead you must pass it to [`crate::VolumeManager::close_volume`] to close
/// it cleanly.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RawVolume(Handle);

impl RawVolume {
    /// Convert a raw volume into a droppable [`Volume`]
    pub fn to_volume<
        D,
        T,
        const MAX_DIRS: usize,
        const MAX_FILES: usize,
        const MAX_VOLUMES: usize,
    >(
        self,
        volume_mgr: &VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    ) -> Volume<'_, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
    where
        D: crate::BlockDevice,
        T: crate::TimeSource,
    {
        Volume::new(self, volume_mgr)
    }
}

/// A caching layer for block devices
///
/// Caches a single block.
#[derive(Debug)]
pub struct BlockCache<D> {
    block_device: D,
    block: [Block; 1],
    block_idx: Option<BlockIdx>,
}

impl<D> BlockCache<D>
where
    D: BlockDevice,
{
    /// Create a new block cache
    pub fn new(block_device: D) -> Self {
        BlockCache {
            block_device,
            block: [Block::new()],
            block_idx: None,
        }
    }

    /// Read a block, and return a reference to it.
    pub fn read(&mut self, block_idx: BlockIdx) -> Result<&Block, D::Error> {
        if self.block_idx != Some(block_idx) {
            self.block_idx = None;
            self.block_device.read(&mut self.block, block_idx)?;
            self.block_idx = Some(block_idx);
        }
        Ok(&self.block[0])
    }

    /// Read a block, and return a reference to it.
    pub fn read_mut(&mut self, block_idx: BlockIdx) -> Result<&mut Block, D::Error> {
        if self.block_idx != Some(block_idx) {
            self.block_idx = None;
            self.block_device.read(&mut self.block, block_idx)?;
            self.block_idx = Some(block_idx);
        }
        Ok(&mut self.block[0])
    }

    /// Write back a block you read with [`Self::read_mut`] and then modified.
    pub fn write_back(&mut self) -> Result<(), D::Error> {
        self.block_device.write(
            &self.block,
            self.block_idx.expect("write_back with no read"),
        )
    }

    /// Write back a block you read with [`Self::read_mut`] and then modified, but to two locations.
    ///
    /// This is useful for updating two File Allocation Tables.
    pub fn write_back_with_duplicate(&mut self, duplicate: BlockIdx) -> Result<(), D::Error> {
        self.block_device.write(
            &self.block,
            self.block_idx.expect("write_back with no read"),
        )?;
        self.block_device.write(&self.block, duplicate)?;
        Ok(())
    }

    /// Access a blank sector
    pub fn blank_mut(&mut self, block_idx: BlockIdx) -> &mut Block {
        self.block_idx = Some(block_idx);
        self.block[0].fill(0);
        &mut self.block[0]
    }

    /// Access the block device
    pub fn block_device(&mut self) -> &mut D {
        // invalidate the cache
        self.block_idx = None;
        // give them the block device
        &mut self.block_device
    }

    /// Get the block device back
    pub fn free(self) -> D {
        self.block_device
    }
}

/// A handle for an open volume on disk, which closes on drop.
///
/// In contrast to a `RawVolume`, a `Volume` holds a mutable reference to its
/// parent `VolumeManager`, which restricts which operations you can perform.
///
/// If you drop a value of this type, it closes the volume automatically, but
/// any error that may occur will be ignored. To handle potential errors, use
/// the [`Volume::close`] method.
pub struct Volume<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
where
    D: crate::BlockDevice,
    T: crate::TimeSource,
{
    raw_volume: RawVolume,
    volume_mgr: &'a VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
}

impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    Volume<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: crate::BlockDevice,
    T: crate::TimeSource,
{
    /// Create a new `Volume` from a `RawVolume`
    pub fn new(
        raw_volume: RawVolume,
        volume_mgr: &'a VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    ) -> Self {
        Volume {
            raw_volume,
            volume_mgr,
        }
    }

    /// Open the volume's root directory.
    ///
    /// You can then read the directory entries with `iterate_dir`, or you can
    /// use `open_file_in_dir`.
    pub fn open_root_dir(
        &self,
    ) -> Result<crate::Directory<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>, Error<D::Error>> {
        let d = self.volume_mgr.open_root_dir(self.raw_volume)?;
        Ok(d.to_directory(self.volume_mgr))
    }

    /// Convert back to a raw volume
    pub fn to_raw_volume(self) -> RawVolume {
        let v = self.raw_volume;
        core::mem::forget(self);
        v
    }

    /// Consume the `Volume` handle and close it. The behavior of this is similar
    /// to using [`core::mem::drop`] or letting the `Volume` go out of scope,
    /// except this lets the user handle any errors that may occur in the process,
    /// whereas when using drop, any errors will be discarded silently.
    pub fn close(self) -> Result<(), Error<D::Error>> {
        let result = self.volume_mgr.close_volume(self.raw_volume);
        core::mem::forget(self);
        result
    }
}

impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize> Drop
    for Volume<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: crate::BlockDevice,
    T: crate::TimeSource,
{
    fn drop(&mut self) {
        _ = self.volume_mgr.close_volume(self.raw_volume)
    }
}

impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    core::fmt::Debug for Volume<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: crate::BlockDevice,
    T: crate::TimeSource,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Volume({})", self.raw_volume.0.0)
    }
}

#[cfg(feature = "defmt-log")]
impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    defmt::Format for Volume<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: crate::BlockDevice,
    T: crate::TimeSource,
{
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "Volume({})", self.raw_volume.0.0)
    }
}

/// Internal information about a Volume
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct VolumeInfo {
    /// Handle for this volume.
    raw_volume: RawVolume,
    /// Which volume (i.e. partition) we opened on the disk
    idx: VolumeIdx,
    /// What kind of volume this is
    volume_type: VolumeType,
}

/// This enum holds the data for the various different types of filesystems we
/// support.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, PartialEq, Eq)]
pub enum VolumeType {
    /// FAT16/FAT32 formatted volumes.
    Fat(FatVolume),
}

/// A number which identifies a volume (or partition) on a disk.
///
/// `VolumeIdx(0)` is the first primary partition on an MBR partitioned disk.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct VolumeIdx(pub usize);

/// Marker for a FAT32 partition. Sometimes also use for FAT16 formatted
/// partitions.
const PARTITION_ID_FAT32_LBA: u8 = 0x0C;
/// Marker for a FAT16 partition with LBA. Seen on a Raspberry Pi SD card.
const PARTITION_ID_FAT16_LBA: u8 = 0x0E;
/// Marker for a FAT16 partition. Seen on a card formatted with the official
/// SD-Card formatter.
const PARTITION_ID_FAT16: u8 = 0x06;
/// Marker for a FAT16 partition smaller than 32MB. Seen on the wowki simulated
/// microsd card
const PARTITION_ID_FAT16_SMALL: u8 = 0x04;
/// Marker for a FAT32 partition. What Macosx disk utility (and also SD-Card formatter?)
/// use.
const PARTITION_ID_FAT32_CHS_LBA: u8 = 0x0B;

// ****************************************************************************
//
// Unit Tests
//
// ****************************************************************************

// None

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
