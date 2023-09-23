use crate::{
    filesystem::{ClusterId, DirEntry, SearchId},
    Volume,
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
pub struct File(pub(crate) SearchId);

/// Internal metadata about an open file
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub(crate) struct FileInfo {
    /// Unique ID for this file
    pub(crate) file_id: File,
    /// The unique ID for the volume this directory is on
    pub(crate) volume_id: Volume,
    /// The current cluster, and how many bytes that short-cuts us
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
        if offset <= self.entry.size {
            self.current_offset = offset;
            if offset < self.current_cluster.0 {
                // Back to start
                self.current_cluster = (0, self.entry.cluster);
            }
            Ok(())
        } else {
            Err(FileError::InvalidOffset)
        }
    }

    /// Seek to a new position in the file, relative to the end of the file.
    pub fn seek_from_end(&mut self, offset: u32) -> Result<(), FileError> {
        if offset <= self.entry.size {
            self.current_offset = self.entry.size - offset;
            if offset < self.current_cluster.0 {
                // Back to start
                self.current_cluster = (0, self.entry.cluster);
            }
            Ok(())
        } else {
            Err(FileError::InvalidOffset)
        }
    }

    /// Seek to a new position in the file, relative to the current position.
    pub fn seek_from_current(&mut self, offset: i32) -> Result<(), FileError> {
        let new_offset = i64::from(self.current_offset) + i64::from(offset);
        if new_offset >= 0 && new_offset <= i64::from(self.entry.size) {
            self.current_offset = new_offset as u32;
            Ok(())
        } else {
            Err(FileError::InvalidOffset)
        }
    }

    /// Amount of file left to read.
    pub fn left(&self) -> u32 {
        self.entry.size - self.current_offset
    }

    /// Update the file's length.
    pub(crate) fn update_length(&mut self, new: u32) {
        self.entry.size = new;
        self.entry.size = new;
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
