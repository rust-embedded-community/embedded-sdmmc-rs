//! embedded-sdmmc-rs - Generic File System
//!
//! Implements generic file system components

use super::Volume;

pub struct Directory<'a> {
    _parent: &'a Volume,
}

pub struct DirEntry {
    pub name: [u8; 11],
    pub mtine: u32,
    pub ctime: u32,
    pub attributes: u8,
}

pub struct File<'a> {
    _parent: &'a Volume,
    _offset: u32,
}

