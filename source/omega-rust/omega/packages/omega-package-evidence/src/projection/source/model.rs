//! Compiler-private rows awaiting canonical source finalization.
//!
//! These carriers retain compiler handles and source spans only while checked
//! state is projected. They must never enter the stable evidence vocabulary or
//! canonical persistence boundary.

use crate::evidence::{
    PackageReviewDangerousAuthority, PackageReviewDangerousAuthoritySlack,
    PackageReviewSemanticDependency, PackageReviewSourceLocationRole,
};
use psi_symbols::SymbolHandle;

/// Pairing between one semantic review row and the exact declaration that
/// produced it. Canonical sorting must move both together; source projection
/// may never rediscover the declaration from reduced row identity.
#[derive(Debug, Clone)]
pub(crate) struct ProjectedReviewRow<Row> {
    pub(crate) row: Row,
    pub(crate) declaration: SymbolHandle,
    pub(crate) nested_source_locations: Vec<ProjectedNestedSourceLocation>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProjectedNestedSourceLocation {
    pub(crate) source_span: psi_source::SourceSpan,
    pub(crate) role: PackageReviewSourceLocationRole,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectedDangerousAuthorityRow {
    pub(crate) row: PackageReviewDangerousAuthority,
    pub(crate) declaration: SymbolHandle,
    pub(crate) exposures: Vec<SymbolHandle>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectedDangerousAuthoritySlackRow {
    pub(crate) row: PackageReviewDangerousAuthoritySlack,
    pub(crate) authority_declaration: SymbolHandle,
    pub(crate) callable_declaration: SymbolHandle,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectedSemanticDependencyRow {
    pub(crate) row: PackageReviewSemanticDependency,
    pub(crate) consumer_declarations: Vec<SymbolHandle>,
    pub(crate) dependency_declarations: Vec<SymbolHandle>,
}
