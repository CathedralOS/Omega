use super::super::{
    authority::{PackageReviewDangerousAuthority, PackageReviewDangerousAuthoritySlack},
    identity::PackageReviewSemanticDependency,
    rows::{PackageReviewCanonicalRowSource, PackageReviewSourceLocationRole},
};
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewCanonicalRowSources {
    pub(crate) public_traits: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_conformances: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_domains: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_propositions: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_consts: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_operators: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_data: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) representation_tcb: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) semantic_dependencies: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) callables: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) external_executable_supply: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) dangerous_authorities: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) dangerous_authority_slack: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) selected_provider_set: PackageReviewCanonicalRowSource,
}

/// Compiler-internal pairing between one semantic review row and the exact
/// declaration that produced it. Canonical sorting must move both together;
/// source projection may never rediscover the declaration from reduced row
/// identity.
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
