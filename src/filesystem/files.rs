use crate::filesystem::{Cluster, DirEntry};

/// Represents an open file on disk.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Debug)]
pub struct File {
    /// The starting point of the file.
    pub(crate) starting_cluster: Cluster,
    /// The current cluster, and how many bytes that short-cuts us
    pub(crate) current_cluster: (u32, Cluster),
    /// How far through the file we've read (in bytes).
    pub(crate) current_offset: u32,
    /// The length of the file, in bytes.
    pub(crate) length: u32,
    /// What mode the file was opened in
    pub(crate) mode: Mode,
    /// DirEntry of this file
    pub(crate) entry: DirEntry,
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

impl File {
    /// Are we at the end of the file?
    pub fn eof(&self) -> bool {
        self.current_offset == self.length
    }

    /// How long is the file?
    pub fn length(&self) -> u32 {
        self.length
    }

    /// Seek to a new position in the file, relative to the start of the file.
    pub fn seek_from_start(&mut self, offset: u32) -> Result<(), FileError> {
        if offset <= self.length {
            self.current_offset = offset;
            if offset < self.current_cluster.0 {
                // Back to start
                self.current_cluster = (0, self.starting_cluster);
            }
            Ok(())
        } else {
            Err(FileError::InvalidOffset)
        }
    }

    /// Seek to a new position in the file, relative to the end of the file.
    pub fn seek_from_end(&mut self, offset: u32) -> Result<(), FileError> {
        if offset <= self.length {
            self.current_offset = self.length - offset;
            if offset < self.current_cluster.0 {
                // Back to start
                self.current_cluster = (0, self.starting_cluster);
            }
            Ok(())
        } else {
            Err(FileError::InvalidOffset)
        }
    }

    /// Seek to a new position in the file, relative to the current position.
    pub fn seek_from_current(&mut self, offset: i32) -> Result<(), FileError> {
        let new_offset = i64::from(self.current_offset) + i64::from(offset);
        if new_offset >= 0 && new_offset <= i64::from(self.length) {
            self.current_offset = new_offset as u32;
            Ok(())
        } else {
            Err(FileError::InvalidOffset)
        }
    }

    /// Amount of file left to read.
    pub fn left(&self) -> u32 {
        self.length - self.current_offset
    }

    pub(crate) fn update_length(&mut self, new: u32) {
        self.length = new;
        self.entry.size = new;
    }
}
