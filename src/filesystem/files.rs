use crate::{
    filesystem::{ClusterId, DirEntry, SearchId},
    Error, RawVolume, VolumeManager,
};

/// Represents an open file on disk.
///
/// Do NOT drop this object! It doesn't hold a reference to the Volume Manager
/// it was created from and cannot update the directory entry if you drop it.
/// Additionally, the VolumeManager will think you still have the file open if
/// you just drop it, and it won't let you open the file again.
///
/// Instead you must pass it to [`crate::VolumeManager::close_file`] to close it
/// cleanly.
///
/// If you want your files to close themselves on drop, create your own File
/// type that wraps this one and also holds a `VolumeManager` reference. You'll
/// then also need to put your `VolumeManager` in some kind of Mutex or RefCell,
/// and deal with the fact you can't put them both in the same struct any more
/// because one refers to the other. Basically, it's complicated and there's a
/// reason we did it this way.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RawFile(pub(crate) SearchId);

impl RawFile {
    /// Convert a raw file into a droppable [`File`]
    pub fn to_file<D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>(
        self,
        volume_mgr: &mut VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    ) -> File<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
    where
        D: crate::BlockDevice,
        T: crate::TimeSource,
    {
        File::new(self, volume_mgr)
    }
}

/// Represents an open file on disk.
///
/// In contrast to a `RawFile`, a `File`  holds a mutable reference to its
/// parent `VolumeManager`, which restricts which operations you can perform.
///
/// If you drop a value of this type, it closes the file automatically, and but
/// error that may occur will be ignored. To handle potential errors, use
/// the [`File::close`] method.
pub struct File<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
where
    D: crate::BlockDevice,
    T: crate::TimeSource,
{
    raw_file: RawFile,
    volume_mgr: &'a mut VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
}

impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    File<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: crate::BlockDevice,
    T: crate::TimeSource,
{
    /// Create a new `File` from a `RawFile`
    pub fn new(
        raw_file: RawFile,
        volume_mgr: &'a mut VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    ) -> File<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES> {
        File {
            raw_file,
            volume_mgr,
        }
    }

    /// Read from the file
    ///
    /// Returns how many bytes were read, or an error.
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, crate::Error<D::Error>> {
        self.volume_mgr.read(self.raw_file, buffer)
    }

    /// Write to the file
    pub fn write(&mut self, buffer: &[u8]) -> Result<(), crate::Error<D::Error>> {
        self.volume_mgr.write(self.raw_file, buffer)
    }

    /// Check if a file is at End Of File.
    pub fn is_eof(&self) -> bool {
        self.volume_mgr
            .file_eof(self.raw_file)
            .expect("Corrupt file ID")
    }

    /// Seek a file with an offset from the current position.
    pub fn seek_from_current(&mut self, offset: i32) -> Result<(), crate::Error<D::Error>> {
        self.volume_mgr
            .file_seek_from_current(self.raw_file, offset)
    }

    /// Seek a file with an offset from the start of the file.
    pub fn seek_from_start(&mut self, offset: u32) -> Result<(), crate::Error<D::Error>> {
        self.volume_mgr.file_seek_from_start(self.raw_file, offset)
    }

    /// Seek a file with an offset back from the end of the file.
    pub fn seek_from_end(&mut self, offset: u32) -> Result<(), crate::Error<D::Error>> {
        self.volume_mgr.file_seek_from_end(self.raw_file, offset)
    }

    /// Get the length of a file
    pub fn length(&self) -> u32 {
        self.volume_mgr
            .file_length(self.raw_file)
            .expect("Corrupt file ID")
    }

    /// Get the current offset of a file
    pub fn offset(&self) -> u32 {
        self.volume_mgr
            .file_offset(self.raw_file)
            .expect("Corrupt file ID")
    }

    /// Convert back to a raw file
    pub fn to_raw_file(self) -> RawFile {
        let f = self.raw_file;
        core::mem::forget(self);
        f
    }

    /// Flush any written data by updating the directory entry.
    pub fn flush(&mut self) -> Result<(), Error<D::Error>> {
        self.volume_mgr.flush_file(self.raw_file)
    }

    /// Consume the `File` handle and close it. The behavior of this is similar
    /// to using [`core::mem::drop`] or letting the `File` go out of scope,
    /// except this lets the user handle any errors that may occur in the process,
    /// whereas when using drop, any errors will be discarded silently.
    pub fn close(self) -> Result<(), Error<D::Error>> {
        let result = self.volume_mgr.close_file(self.raw_file);
        core::mem::forget(self);
        result
    }
}

impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize> Drop
    for File<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: crate::BlockDevice,
    T: crate::TimeSource,
{
    fn drop(&mut self) {
        _ = self.volume_mgr.close_file(self.raw_file);
    }
}

impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    core::fmt::Debug for File<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: crate::BlockDevice,
    T: crate::TimeSource,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "File({})", self.raw_file.0 .0)
    }
}

#[cfg(feature = "defmt-log")]
impl<'a, D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    defmt::Format for File<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: crate::BlockDevice,
    T: crate::TimeSource,
{
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "File({})", self.raw_file.0 .0)
    }
}

/// Errors related to file operations
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileError {
    /// Tried to use an invalid offset.
    InvalidOffset,
}

/// The different ways we can open a file.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
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

/// Internal metadata about an open file
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub(crate) struct FileInfo {
    /// Unique ID for this file
    pub(crate) file_id: RawFile,
    /// The unique ID for the volume this directory is on
    pub(crate) volume_id: RawVolume,
    /// The last cluster we accessed, and how many bytes that short-cuts us.
    ///
    /// This saves us walking from the very start of the FAT chain when we move
    /// forward through a file.
    pub(crate) current_cluster: (u32, ClusterId),
    /// How far through the file we've read (in bytes).
    pub(crate) current_offset: u32,
    /// What mode the file was opened in
    pub(crate) mode: Mode,
    /// DirEntry of this file
    pub(crate) entry: DirEntry,
    /// Did we write to this file?
    pub(crate) dirty: bool,
}

impl FileInfo {
    /// Are we at the end of the file?
    pub fn eof(&self) -> bool {
        self.current_offset == self.entry.size
    }

    /// How long is the file?
    pub fn length(&self) -> u32 {
        self.entry.size
    }

    /// Seek to a new position in the file, relative to the start of the file.
    pub fn seek_from_start(&mut self, offset: u32) -> Result<(), FileError> {
        if offset > self.entry.size {
            return Err(FileError::InvalidOffset);
        }
        self.current_offset = offset;
        Ok(())
    }

    /// Seek to a new position in the file, relative to the end of the file.
    pub fn seek_from_end(&mut self, offset: u32) -> Result<(), FileError> {
        if offset > self.entry.size {
            return Err(FileError::InvalidOffset);
        }
        self.current_offset = self.entry.size - offset;
        Ok(())
    }

    /// Seek to a new position in the file, relative to the current position.
    pub fn seek_from_current(&mut self, offset: i32) -> Result<(), FileError> {
        let new_offset = i64::from(self.current_offset) + i64::from(offset);
        if new_offset < 0 || new_offset > i64::from(self.entry.size) {
            return Err(FileError::InvalidOffset);
        }
        self.current_offset = new_offset as u32;
        Ok(())
    }

    /// Amount of file left to read.
    pub fn left(&self) -> u32 {
        self.entry.size - self.current_offset
    }

    /// Update the file's length.
    pub(crate) fn update_length(&mut self, new: u32) {
        self.entry.size = new;
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
