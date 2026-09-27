//! Copy projections preserve the vocabulary of the admitted Terminal operation.
//! A case-qualified projection cannot be flattened into field names: case
//! payloads overlay one another, and the selected case is part of its identity.

use semantic_vocabulary::CanonicalStructuralPathSegment;
use terminal_psi::StructuralPathSegment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralCopyPath {
    Ordinary(Vec<StructuralPathSegment>),
    Case(Vec<CanonicalStructuralPathSegment>),
}

impl From<Vec<StructuralPathSegment>> for StructuralCopyPath {
    fn from(path: Vec<StructuralPathSegment>) -> Self {
        Self::Ordinary(path)
    }
}
