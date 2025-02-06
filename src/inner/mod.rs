// ****************************************************************************
//
// Imports
//
// ****************************************************************************

#![deny(missing_docs)]
// The compiler warning for `async fn` in public traits isn't relevant for embedded,
// so silence it.
// https://github.com/rust-embedded/embedded-hal/pull/515#issuecomment-1763525962
#![allow(async_fn_in_trait)]

pub mod blockdevice;
pub mod fat;
pub mod filesystem;
pub mod sdcard;

use core::fmt::Debug;
use embedded_io::ErrorKind;
use filesystem::Handle;

use super::{bisync, only_sync};

#[doc(inline)]
pub use blockdevice::{Block, BlockCache, BlockCount, BlockDevice, BlockIdx};

#[doc(inline)]
pub use fat::{FatVolume, VolumeName};

#[doc(inline)]
pub use filesystem::{
    Attributes, ClusterId, DirEntry, Directory, File, FilenameError, LfnBuffer, Mode, RawDirectory,
    RawFile, ShortFileName, TimeSource, Timestamp, MAX_FILE_SIZE,
};

use filesystem::DirectoryInfo;

#[doc(inline)]
pub use sdcard::Error as SdCardError;

#[doc(inline)]
pub use sdcard::SdCard;

mod volume_mgr;
#[doc(inline)]
pub use volume_mgr::VolumeManager;

// ****************************************************************************
//
// Public Types
//
// ****************************************************************************

/// All the ways the functions in this crate can fail.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub enum Error<E>
where
    E: core::fmt::Debug,
{
    /// The underlying block device threw an error.
    DeviceError(E),
    /// The filesystem is badly formatted (or this code is buggy).
    FormatError(&'static str),
    /// The given `VolumeIdx` was bad,
    NoSuchVolume,
    /// The given filename was bad
    FilenameError(FilenameError),
    /// Out of memory opening volumes
    TooManyOpenVolumes,
    /// Out of memory opening directories
    TooManyOpenDirs,
    /// Out of memory opening files
    TooManyOpenFiles,
    /// Bad handle given
    BadHandle,
    /// That file or directory doesn't exist
    NotFound,
    /// You can't open a file twice or delete an open file
    FileAlreadyOpen,
    /// You can't open a directory twice
    DirAlreadyOpen,
    /// You can't open a directory as a file
    OpenedDirAsFile,
    /// You can't open a file as a directory
    OpenedFileAsDir,
    /// You can't delete a directory as a file
    DeleteDirAsFile,
    /// You can't close a volume with open files or directories
    VolumeStillInUse,
    /// You can't open a volume twice
    VolumeAlreadyOpen,
    /// We can't do that yet
    Unsupported,
    /// Tried to read beyond end of file
    EndOfFile,
    /// Found a bad cluster
    BadCluster,
    /// Error while converting types
    ConversionError,
    /// The device does not have enough space for the operation
    NotEnoughSpace,
    /// Cluster was not properly allocated by the library
    AllocationError,
    /// Jumped to free space during FAT traversing
    UnterminatedFatChain,
    /// Tried to open Read-Only file with write mode
    ReadOnly,
    /// Tried to create an existing file
    FileAlreadyExists,
    /// Bad block size - only 512 byte blocks supported
    BadBlockSize(u16),
    /// Bad offset given when seeking
    InvalidOffset,
    /// Disk is full
    DiskFull,
    /// A directory with that name already exists
    DirAlreadyExists,
    /// The filesystem tried to gain a lock whilst already locked.
    ///
    /// This is either a bug in the filesystem, or you tried to access the
    /// filesystem API from inside a directory iterator (that isn't allowed).
    LockError,
}

impl<E: Debug> embedded_io::Error for Error<E> {
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
            | Error::DeleteDirAsFile
            | Error::BadCluster
            | Error::ConversionError
            | Error::UnterminatedFatChain => ErrorKind::InvalidData,
            Error::Unsupported | Error::BadBlockSize(_) => ErrorKind::Unsupported,
            Error::ReadOnly => ErrorKind::PermissionDenied,
            Error::FileAlreadyExists | Error::DirAlreadyExists => ErrorKind::AlreadyExists,
        }
    }
}

impl<E> From<E> for Error<E>
where
    E: core::fmt::Debug,
{
    fn from(value: E) -> Error<E> {
        Error::DeviceError(value)
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
/// Instead you must pass it to [`VolumeManager::close_volume`] to close
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
    ) -> Volume<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        Volume::new(self, volume_mgr)
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
    D: BlockDevice,
    T: TimeSource,
{
    raw_volume: RawVolume,
    volume_mgr: &'a VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
}

#[bisync]
impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    Volume<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: BlockDevice,
    T: TimeSource,
{
    /// Create a new `Volume` from a `RawVolume`
    pub fn new(
        raw_volume: RawVolume,
        volume_mgr: &'a VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    ) -> Volume<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES> {
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
    ) -> Result<Directory<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>, Error<D::Error>> {
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
    pub async fn close(self) -> Result<(), Error<D::Error>> {
        let result = self.volume_mgr.close_volume(self.raw_volume).await;
        core::mem::forget(self);
        result
    }
}

// async drop does not yet exist :(
#[only_sync]
impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize> Drop
    for Volume<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: BlockDevice,
    T: TimeSource,
{
    fn drop(&mut self) {
        _ = self.volume_mgr.close_volume(self.raw_volume)
    }
}

impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    core::fmt::Debug for Volume<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: BlockDevice,
    T: TimeSource,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Volume({})", self.raw_volume.0 .0)
    }
}

#[cfg(feature = "defmt-log")]
impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    defmt::Format for Volume<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: BlockDevice,
    T: TimeSource,
{
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "Volume({})", self.raw_volume.0 .0)
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
