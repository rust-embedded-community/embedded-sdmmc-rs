use super::super::super::bisync;
use super::super::blockdevice::BlockIdx;
use super::super::fat::{FatType, OnDiskDirEntry};
use super::super::filesystem::{
    Attributes, ClusterId, Handle, LfnBuffer, ShortFileName, Timestamp,
};
use super::super::{BlockDevice, File, Mode, TimeSource};
use super::super::{Error, RawVolume, VolumeManager};

use super::ToShortFileName;

/// A directory entry, which tells you about other files and directories.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, PartialEq, Eq, Clone)]
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
    pub cluster: ClusterId,
    /// The size of the file in bytes.
    pub size: u32,
    /// The disk block of this entry
    pub entry_block: BlockIdx,
    /// The offset on its block (in bytes)
    pub entry_offset: u32,
}

/// A handle for an open directory on disk.
///
/// Do NOT drop this object! It doesn't hold a reference to the Volume Manager
/// it was created from and if you drop it, the VolumeManager will think you
/// still have the directory open, and it won't let you open the directory
/// again.
///
/// Instead you must pass it to [`VolumeManager::close_dir`] to close it
/// cleanly.
///
/// If you want your directories to close themselves on drop, create your own
/// `Directory` type that wraps this one and also holds a `VolumeManager`
/// reference. You'll then also need to put your `VolumeManager` in some kind of
/// Mutex or RefCell, and deal with the fact you can't put them both in the same
/// struct any more because one refers to the other. Basically, it's complicated
/// and there's a reason we did it this way.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RawDirectory(pub(crate) Handle);

impl RawDirectory {
    /// Convert a raw directory into a droppable [`Directory`]
    pub fn to_directory<
        D,
        T,
        const MAX_DIRS: usize,
        const MAX_FILES: usize,
        const MAX_VOLUMES: usize,
    >(
        self,
        volume_mgr: &VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    ) -> Directory<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
    where
        D: BlockDevice,
        T: TimeSource,
    {
        Directory::new(self, volume_mgr)
    }
}

/// A handle for an open directory on disk, which closes on drop.
///
/// In contrast to a `RawDirectory`, a `Directory` holds a mutable reference to
/// its parent `VolumeManager`, which restricts which operations you can perform.
///
/// If you drop a value of this type, it closes the directory automatically, but
/// any error that may occur will be ignored. To handle potential errors, use
/// the [`Directory::close`] method.
pub struct Directory<
    'a,
    D,
    T,
    const MAX_DIRS: usize,
    const MAX_FILES: usize,
    const MAX_VOLUMES: usize,
> where
    D: BlockDevice,
    T: TimeSource,
{
    raw_directory: RawDirectory,
    volume_mgr: &'a VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
}

#[bisync]
impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    Directory<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: BlockDevice,
    T: TimeSource,
{
    /// Create a new `Directory` from a `RawDirectory`
    pub fn new(
        raw_directory: RawDirectory,
        volume_mgr: &'a VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    ) -> Directory<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES> {
        Directory {
            raw_directory,
            volume_mgr,
        }
    }

    /// Open a directory.
    ///
    /// You can then read the directory entries with `iterate_dir` and `open_file_in_dir`.
    pub async fn open_dir<N>(
        &self,
        name: N,
    ) -> Result<Directory<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>, Error<D::Error>>
    where
        N: ToShortFileName,
    {
        let d = self.volume_mgr.open_dir(self.raw_directory, name).await?;
        Ok(d.to_directory(self.volume_mgr))
    }

    /// Change to a directory, mutating this object.
    ///
    /// You can then read the directory entries with `iterate_dir` and `open_file_in_dir`.
    pub async fn change_dir<N>(&mut self, name: N) -> Result<(), Error<D::Error>>
    where
        N: ToShortFileName,
    {
        let d = self.volume_mgr.open_dir(self.raw_directory, name).await?;
        self.volume_mgr.close_dir(self.raw_directory).unwrap();
        self.raw_directory = d;
        Ok(())
    }

    /// Look in a directory for a named file.
    pub async fn find_directory_entry<N>(&self, name: N) -> Result<DirEntry, Error<D::Error>>
    where
        N: ToShortFileName,
    {
        self.volume_mgr
            .find_directory_entry(self.raw_directory, name).await
    }

    /// Call a callback function for each directory entry in a directory.
    ///
    /// Long File Names will be ignored.
    ///
    /// <div class="warning">
    ///
    /// Do not attempt to call any methods on the VolumeManager or any of its
    /// handles from inside the callback. You will get a lock error because the
    /// object is already locked in order to do the iteration.
    ///
    /// </div>
    pub async fn iterate_dir<F>(&self, func: F) -> Result<(), Error<D::Error>>
    where
        F: FnMut(&DirEntry),
    {
        self.volume_mgr.iterate_dir(self.raw_directory, func).await
    }

    /// Call a callback function for each directory entry in a directory, and
    /// process Long File Names.
    ///
    /// You must supply a [`LfnBuffer`] this API can use to temporarily hold the
    /// Long File Name. If you pass one that isn't large enough, any Long File
    /// Names that don't fit will be ignored and presented as if they only had a
    /// Short File Name.
    ///
    /// <div class="warning">
    ///
    /// Do not attempt to call any methods on the VolumeManager or any of its
    /// handles from inside the callback. You will get a lock error because the
    /// object is already locked in order to do the iteration.
    ///
    /// </div>
    pub async fn iterate_dir_lfn<F>(
        &self,
        lfn_buffer: &mut LfnBuffer<'_>,
        func: F,
    ) -> Result<(), Error<D::Error>>
    where
        F: FnMut(&DirEntry, Option<&str>),
    {
        self.volume_mgr
            .iterate_dir_lfn(self.raw_directory, lfn_buffer, func).await
    }

    /// Open a file with the given full path. A file can only be opened once.
    pub async fn open_file_in_dir<N>(
        &self,
        name: N,
        mode: Mode,
    ) -> Result<File<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>, Error<D::Error>>
    where
        N: super::ToShortFileName,
    {
        let f = self
            .volume_mgr
            .open_file_in_dir(self.raw_directory, name, mode).await?;
        Ok(f.to_file(self.volume_mgr))
    }

    /// Delete a closed file with the given filename, if it exists.
    pub async fn delete_file_in_dir<N>(&self, name: N) -> Result<(), Error<D::Error>>
    where
        N: ToShortFileName,
    {
        self.volume_mgr.delete_file_in_dir(self.raw_directory, name).await
    }

    /// Make a directory inside this directory
    pub async fn make_dir_in_dir<N>(&self, name: N) -> Result<(), Error<D::Error>>
    where
        N: ToShortFileName,
    {
        self.volume_mgr.make_dir_in_dir(self.raw_directory, name).await
    }

    /// Convert back to a raw directory
    pub fn to_raw_directory(self) -> RawDirectory {
        let d = self.raw_directory;
        core::mem::forget(self);
        d
    }

    /// Consume the `Directory` handle and close it. The behavior of this is similar
    /// to using [`core::mem::drop`] or letting the `Directory` go out of scope,
    /// except this lets the user handle any errors that may occur in the process,
    /// whereas when using drop, any errors will be discarded silently.
    pub fn close(self) -> Result<(), Error<D::Error>> {
        let result = self.volume_mgr.close_dir(self.raw_directory);
        core::mem::forget(self);
        result
    }
}

impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize> Drop
    for Directory<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: BlockDevice,
    T: TimeSource,
{
    fn drop(&mut self) {
        _ = self.volume_mgr.close_dir(self.raw_directory)
    }
}

impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    core::fmt::Debug for Directory<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: BlockDevice,
    T: TimeSource,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Directory({})", self.raw_directory.0 .0)
    }
}

#[cfg(feature = "defmt-log")]
impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    defmt::Format for Directory<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: BlockDevice,
    T: TimeSource,
{
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "Directory({})", self.raw_directory.0 .0)
    }
}

/// Holds information about an open file on disk
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub(crate) struct DirectoryInfo {
    /// The handle for this directory.
    pub(crate) raw_directory: RawDirectory,
    /// The handle for the volume this directory is on
    pub(crate) raw_volume: RawVolume,
    /// The starting point of the directory listing.
    pub(crate) cluster: ClusterId,
}

impl DirEntry {
    pub(crate) fn serialize(&self, fat_type: FatType) -> [u8; OnDiskDirEntry::LEN] {
        let mut data = [0u8; OnDiskDirEntry::LEN];
        data[0..11].copy_from_slice(&self.name.contents);
        data[11] = self.attributes.0;
        // 12: Reserved. Must be set to zero
        // 13: CrtTimeTenth, not supported, set to zero
        data[14..18].copy_from_slice(&self.ctime.serialize_to_fat()[..]);
        // 0 + 18: LastAccDate, not supported, set to zero
        let cluster_number = self.cluster.0;
        let cluster_hi = if fat_type == FatType::Fat16 {
            [0u8; 2]
        } else {
            // Safe due to the AND operation
            (((cluster_number >> 16) & 0x0000_FFFF) as u16).to_le_bytes()
        };
        data[20..22].copy_from_slice(&cluster_hi[..]);
        data[22..26].copy_from_slice(&self.mtime.serialize_to_fat()[..]);
        // Safe due to the AND operation
        let cluster_lo = ((cluster_number & 0x0000_FFFF) as u16).to_le_bytes();
        data[26..28].copy_from_slice(&cluster_lo[..]);
        data[28..32].copy_from_slice(&self.size.to_le_bytes()[..]);
        data
    }

    pub(crate) fn new(
        name: ShortFileName,
        attributes: Attributes,
        cluster: ClusterId,
        ctime: Timestamp,
        entry_block: BlockIdx,
        entry_offset: u32,
    ) -> Self {
        Self {
            name,
            mtime: ctime,
            ctime,
            attributes,
            cluster,
            size: 0,
            entry_block,
            entry_offset,
        }
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
