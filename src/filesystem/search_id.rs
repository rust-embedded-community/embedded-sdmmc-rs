use core::num::Wrapping;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
/// Unique ID used to search for files and directories in the open File/Directory lists
pub struct SearchId(pub(crate) u32);

/// ID generator intented to be used in a static context.
///
/// This object will always return a different ID.
pub struct IdGenerator {
    next_id: Wrapping<u32>,
}

impl IdGenerator {
    /// Create a new [`IdGenerator`].
    pub const fn new() -> Self {
        Self {
            next_id: Wrapping(0),
        }
    }

    /// Generate a new, unique [`SearchId`].
    pub fn get(&mut self) -> SearchId {
        let id = self.next_id;
        self.next_id += 1;
        SearchId(id.0)
    }
}
