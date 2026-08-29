//! Turn declared source requests into one validated package closure.
//!
//! [`package`] binds immutable source snapshots to package declarations.
//! [`graph`] follows those declared dependencies and reconciles their complete
//! identity and reachability. Read them in that order.

pub mod graph;
pub mod package;

#[cfg(test)]
pub(crate) use graph::resolve_external_local_package_closure;
pub use graph::{
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
    resolve_workspace_package_closure_in_context_with_storage,
    resolve_workspace_package_closure_with_storage,
};
pub use omega_package_source::{
    AliasName, ExternalLocalLineage, ExternalSourceContext, GenericGitLineage, GitCommitId,
    GitExecutableIdentity, GitHubRepositoryLineage, GitLabRepositoryLineage,
    GitNetworkTransferObservation, GitObjectIdAlgorithm, GitSourceRequest, GitSourceRequestError,
    GitSourceResolutionObservation, GitTransport, GitTransportExecutableIdentity,
    GitTransportProfile, GitTreeId, IdentityError, ImmutableSourceResolution, LocalSourceLimits,
    LocalSourceResolutionObservation, PackageKey, PackageName, ResolvedGitSource,
    ResolvedLocalSnapshot, ResolvedLocalSource, SourceContentDigest, SourceLineage,
    SourceResolveError, SourceResolverStorage, WorkspaceLineageIdentity, WorkspaceMemberLineage,
    WorkspaceMemberPath, resolve_git_source_with_storage, resolve_local_source,
    resolve_local_source_snapshot_with_storage,
};
pub use package::{
    PackageSourceCustody, ResolvePackageSourceError, ResolvedPackageSource,
    resolve_external_local_package_source_with_storage,
    resolve_external_local_project_source_with_storage, resolve_git_package_source_with_storage,
    resolve_workspace_member_package_source_with_storage,
};
#[cfg(test)]
pub(crate) use package::{
    resolve_external_local_package_source, resolve_workspace_member_package_source,
};
