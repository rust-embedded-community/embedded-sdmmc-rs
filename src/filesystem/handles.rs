//! Contains the Handles and the HandleGenerator.

use core::num::Wrapping;

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
/// Unique ID used to identify things in the open Volume/File/Directory lists
pub struct Handle(pub(crate) u32);

impl core::fmt::Debug for Handle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#08x}", self.0)
    }
}

/// A Handle Generator.
///
/// This object will always return a different ID.
///
/// Well, it will wrap after `2**32` IDs. But most systems won't open that many
/// files, and if they do, they are unlikely to hold one file open and then
/// open/close `2**32 - 1` others.
#[derive(Debug)]
pub struct HandleGenerator {
    next_id: Wrapping<u32>,
}

impl HandleGenerator {
    /// Create a new generator of Handles.
    pub const fn new(offset: u32) -> Self {
        Self {
            next_id: Wrapping(offset),
        }
    }

    /// Generate a new, unique [`Handle`].
    pub fn generate(&mut self) -> Handle {
        let id = self.next_id;
        self.next_id += 1;
        Handle(id.0)
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
