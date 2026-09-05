//! Explicit canonical ordering policy for each admitted layout shape.

/// Required-stage baseline layout for the currently admitted three-block
/// conditional. This is a visible policy identity, not an optimization level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedFunctionLayoutPolicy {
    EntryThenZeroFallthroughThenNonzeroV1,
    EntryThenNotLessFallthroughThenLessV1,
    SingleEntryBlockV1,
    /// The module contains more than one admitted canonical function shape;
    /// each function derives its own exact block order from its terminator.
    PerFunctionCanonicalShapeV1,
    /// A separate zero-VReg structural roster. Every function has one entry
    /// block containing either `ReturnUnit`, or one unresolved whole-root
    /// `CallUnit` template followed by `ReturnUnit`.
    StructuralUnitCallThenReturnSingleEntryBlockV1,
}
