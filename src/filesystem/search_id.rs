#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
/// Unique ID used to search for files and directories in the open File/Directory lists
pub struct SearchId(pub(crate) u32);

/// ID generator intented to be used in a static context.
///
/// This object will always return a different ID.
pub struct IdGenerator {
    next_id: core::sync::atomic::AtomicU32,
}

impl IdGenerator {
    /// Create a new [`IdGenerator`].
    pub const fn new() -> Self {
        Self {
            next_id: core::sync::atomic::AtomicU32::new(0),
        }
    }

    /// Generate a new, unique [`SearchId`].
    pub fn next(&self) -> SearchId {
        use core::sync::atomic::Ordering;
        let id = self.next_id.load(Ordering::Acquire);
        self.next_id.store(id + 1, Ordering::Release);
        SearchId(id)
    }
}
