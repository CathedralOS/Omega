// Where a case dispatch finds the pointer to a payload.

/// One pointer word between a sum and the storage selection has to load.
///
/// `index_start..index_end` selects this word's runtime indices in the
/// operation's flattened index list. The list is prepared once, on the
/// target operation that projected the sum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetCasePointerSlot {
    pub byte_offset: u32,
    pub index_start: u32,
    pub index_end: u32,
    pub scalar_descriptor: bool,
}
