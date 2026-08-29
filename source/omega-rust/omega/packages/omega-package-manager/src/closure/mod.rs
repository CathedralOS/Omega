//! Complete package-closure resolution.
//!
//! [`traversal`] connects declared workspace, local, and Git requests to source
//! custody. [`reconciliation`] builds the complete dependency closure, [`graph`]
//! validates its shape, and [`subject`] gives that exact closure a canonical
//! review identity.

pub(crate) mod graph;
pub(crate) mod reconciliation;
mod subject;
mod traversal;

pub use graph::{
    PackageClosureValidationError, ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode,
    ResolvedSourceIdentity,
};
pub use reconciliation::{
    DependencyRequestPath, DependencyRequestPathStep, PackageRootSourceRequest,
    PackageSourceClosureConflict, PackageSourceClosureConflictCandidate,
    PackageSourceClosureLimitKind, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    PackageSourceCustody, ResolvedDependencySourceRequest, ResolvedPackageSourceClosure,
    ResolvedPackageSourceRequestSet, ResolvedRootPackageSourceRequest,
};
pub use subject::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
};
#[cfg(test)]
pub(crate) use traversal::resolve_external_local_package_closure;
pub use traversal::{
    ResolveDependencySourceError, ResolveExternalLocalPackageClosureError,
    ResolveGitPackageClosureError, ResolveWorkspacePackageClosureError,
    resolve_external_local_package_closure_with_storage,
    resolve_external_local_project_closure_with_storage, resolve_git_package_closure_with_storage,
    resolve_workspace_package_closure_in_context_with_storage,
    resolve_workspace_package_closure_with_storage,
};
