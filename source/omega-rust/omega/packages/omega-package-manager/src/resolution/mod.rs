//! Turn declared source requests into one validated package closure.
//!
//! [`source`] joins immutable source snapshots to package declarations.
//! [`closure`] follows those declared dependencies and reconciles their complete
//! identity and reachability. Read them in that order.

pub mod closure;
pub mod source;

#[cfg(test)]
pub(crate) use closure::resolve_external_local_package_closure;
pub use closure::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, DependencyRequestPath, DependencyRequestPathStep,
    PackageClosureValidationError, PackageRootSourceRequest, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError, ResolveDependencySourceError,
    ResolveExternalLocalPackageClosureError, ResolveGitPackageClosureError,
    ResolveWorkspacePackageClosureError, ResolvedDependency, ResolvedDependencySourceRequest,
    ResolvedPackageClosure, ResolvedPackageNode, ResolvedPackageSourceClosure,
    ResolvedPackageSourceRequestSet, ResolvedRootPackageSourceRequest, ResolvedSourceIdentity,
    SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION, resolve_external_local_package_closure_with_storage,
    resolve_external_local_project_closure_with_storage, resolve_git_package_closure_with_storage,
    resolve_selected_git_package_closure_with_storage,
    resolve_workspace_package_closure_in_context_with_storage,
    resolve_workspace_package_closure_with_storage,
};
pub use source::{
    GitPackageSourceRequest, PackageSourceCustody, PackageSourceMaterialization,
    PackageSourceNavigation, PackageSourceSelectionEvidence, PackageSourceSelectionEvidenceError,
    ResolvePackageSourceError, ResolvedPackageSource,
    resolve_external_local_package_source_with_storage,
    resolve_external_local_project_source_with_storage, resolve_git_package_source_with_storage,
    resolve_selected_git_package_source_with_storage,
    resolve_workspace_member_package_source_with_storage,
};
#[cfg(test)]
pub(crate) use source::{
    resolve_external_local_package_source, resolve_workspace_member_package_source,
};
