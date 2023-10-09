use core::num::Wrapping;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
/// Unique ID used to search for files and directories in the open Volume/File/Directory lists
pub struct SearchId(pub(crate) u32);

/// A Search ID generator.
///
/// This object will always return a different ID.
///
/// Well, it will wrap after `2**32` IDs. But most systems won't open that many
/// files, and if they do, they are unlikely to hold one file open and then
/// open/close `2**32 - 1` others.
#[derive(Debug)]
pub struct SearchIdGenerator {
    next_id: Wrapping<u32>,
}

impl SearchIdGenerator {
    /// Create a new generator of Search IDs.
    pub const fn new(offset: u32) -> Self {
        Self {
            next_id: Wrapping(offset),
        }
    }

    /// Generate a new, unique [`SearchId`].
    pub fn get(&mut self) -> SearchId {
        let id = self.next_id;
        self.next_id += 1;
        SearchId(id.0)
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
