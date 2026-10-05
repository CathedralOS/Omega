//! Compiler-private rows awaiting canonical source finalization.
//!
//! These carriers retain compiler handles and source spans only while checked
//! state is projected. They must never enter the stable evidence vocabulary or
//! canonical persistence boundary.

use symbols::SymbolHandle;

/// Pairing between one semantic review row and the exact declaration that
/// produced it. Canonical sorting must move both together; source projection
/// may never rediscover the declaration from reduced row identity.
#[derive(Debug, Clone)]
pub(crate) struct ProjectedReviewRow<Row> {
    pub(crate) row: Row,
    pub(crate) declaration: SymbolHandle,
}
