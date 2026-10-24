//! Compiler-private rows awaiting canonical source finalization.
//!
//! These carriers retain compiler handles, source spans, and the exact
//! producing declaration only while checked state is projected. They must never
//! enter the stable evidence vocabulary or canonical persistence boundary.

/// Pairing between one semantic review row and the exact declaration that
/// produced it. Canonical sorting must move both together; a consumer that
/// needs the declaration's own tables reads the carried declaration rather
/// than rediscovering it from the row's reduced identity.
#[derive(Debug, Clone)]
pub(crate) struct ProjectedReviewRow<'a, Row, Declaration> {
    pub(crate) row: Row,
    pub(crate) declaration: &'a Declaration,
}
