//! Package-owned acquired source custody for an actual Build request ledger.
//! This query supplies no host grant and cannot execute or complete a Build.

use crate::PackageCompilationInputs;
use build_declarations::{BuildRequest, DependencyPurpose};
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDependencySelection {
    pub purpose: DependencyPurpose,
    pub request_ordinal: usize,
    pub package: PackageKeyIdentity,
}

/// The package owner compares full source requests with its acquired immutable
/// graph. Aliases and a graph alone cannot authenticate that correspondence.
pub trait BuildDependencySourceJoin: Send + Sync {
    fn resolve_issued_requests(
        &self,
        inputs: &PackageCompilationInputs,
        requests: &[&BuildRequest],
    ) -> Result<Vec<BuildDependencySelection>, String>;
}
