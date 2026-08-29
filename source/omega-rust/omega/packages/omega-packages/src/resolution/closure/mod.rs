//! Complete package-closure resolution.
//!
//! [`sources`] connects declared workspace, local, and Git requests to source
//! custody. [`reconcile`] builds the complete dependency closure, [`graph`]
//! validates its shape, and [`subject`] gives that exact closure a canonical
//! review identity.

pub(crate) mod graph;
pub(crate) mod reconcile;
mod sources;
mod subject;

pub use graph::{
    PackageClosureValidationError, ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode,
    ResolvedSourceIdentity,
};
pub use reconcile::{
    DependencyRequestPath, DependencyRequestPathStep, PackageRootSourceRequest,
    PackageSourceClosureConflict, PackageSourceClosureConflictCandidate,
    PackageSourceClosureLimitKind, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    PackageSourceCustody, ResolvedDependencySourceRequest, ResolvedPackageSourceClosure,
    ResolvedPackageSourceRequestSet, ResolvedRootPackageSourceRequest,
};
pub use sources::{
    ResolveDependencySourceError, ResolveExternalLocalPackageClosureError,
    ResolveGitPackageClosureError, ResolveWorkspacePackageClosureError,
    resolve_external_local_package_closure, resolve_external_local_package_closure_with_storage,
    resolve_external_local_project_closure, resolve_external_local_project_closure_with_storage,
    resolve_git_package_closure, resolve_git_package_closure_with_storage,
    resolve_workspace_package_closure, resolve_workspace_package_closure_in_context,
    resolve_workspace_package_closure_in_context_with_storage,
    resolve_workspace_package_closure_with_storage,
};
pub use subject::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
};
