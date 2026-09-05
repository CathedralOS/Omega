//! Complete dependency-graph construction and identity.
//!
//! [`resolve`] connects declared workspace, local, and Git requests to source
//! custody. [`reconcile`] builds the complete dependency closure, [`model`]
//! owns its validated shape, and [`subject`] gives that exact closure a
//! canonical review subject.

mod model;
pub(crate) mod reconcile;
mod resolve;
mod root_request;
mod subject;

#[cfg(test)]
mod model_tests;

pub use model::{
    PackageClosureValidationError, ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode,
    ResolvedSourceIdentity,
};
pub use reconcile::{
    DependencyRequestPath, DependencyRequestPathStep, ExactTargetPackageSourceClosure,
    PackageSourceClosureConflict, PackageSourceClosureConflictCandidate,
    PackageSourceClosureLimitKind, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    ResolvedDependencySourceRequest, ResolvedPackageSourceClosure, ResolvedPackageSourceRequestSet,
    ResolvedRootPackageSourceRequest,
};
#[cfg(test)]
pub(crate) use resolve::resolve_external_local_package_closure;
pub use resolve::{
    ResolveDependencySourceError, ResolveExternalLocalPackageClosureError,
    ResolveGitPackageClosureError, ResolveWorkspacePackageClosureError,
    resolve_external_local_package_closure_with_storage,
    resolve_external_local_project_closure_with_storage, resolve_git_package_closure_with_storage,
    resolve_git_project_closure_with_storage, resolve_selected_git_package_closure_with_storage,
    resolve_selected_git_project_closure_with_storage,
    resolve_workspace_package_closure_in_context_with_storage,
    resolve_workspace_package_closure_with_storage,
    resolve_workspace_project_closure_in_context_with_storage,
    resolve_workspace_project_closure_with_storage,
};
pub use root_request::PackageRootSourceRequest;
pub use subject::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, CanonicalSourceClosureSubjectRecoveryUsage,
    SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
};
