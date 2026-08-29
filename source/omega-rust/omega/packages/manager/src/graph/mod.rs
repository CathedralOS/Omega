//! Complete dependency-graph construction and identity.
//!
//! [`traversal`] connects declared workspace, local, and Git requests to source
//! custody. [`reconciliation`] builds the complete dependency closure, [`graph`]
//! validates its shape, and [`subject`] gives that exact closure a canonical
//! review identity.

pub(crate) mod reconciliation;
mod root_request;
mod subject;
mod traversal;
pub(crate) mod validation;

pub use reconciliation::{
    DependencyRequestPath, DependencyRequestPathStep, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    ResolvedDependencySourceRequest, ResolvedPackageSourceClosure, ResolvedPackageSourceRequestSet,
    ResolvedRootPackageSourceRequest,
};
pub use root_request::PackageRootSourceRequest;
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
pub use validation::{
    PackageClosureValidationError, ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode,
    ResolvedSourceIdentity,
};
