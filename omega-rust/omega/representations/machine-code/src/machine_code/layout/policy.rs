//! Explicit canonical ordering policy for each admitted layout shape.

/// Required-stage baseline block ordering, not an optimization level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedFunctionLayoutPolicy {
    EntryThenZeroFallthroughThenNonzeroV1,
    EntryThenNotLessFallthroughThenLessV1,
    SingleEntryBlockV1,
    /// Entry-first conditional-fallthrough chains; remaining chain roots follow
    /// the selected block roster. Explicit jumps do not impose adjacency.
    PerFunctionCanonicalShapeV1,
}
