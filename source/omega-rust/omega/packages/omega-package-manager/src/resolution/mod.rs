//! Package identity, hostile acquisition, and complete closure resolution.
//!
//! Read this module from the outside inward:
//! [`identity`] names immutable package sources, [`acquisition`] captures and
//! authenticates hostile local or Git content, [`package`] connects one declared
//! package request to retained source custody, and [`closure`] resolves the
//! complete dependency closure. [`inspection`] exposes the source-inspection
//! command boundary.

pub(crate) mod acquisition;
pub(crate) mod closure;
pub(crate) mod identity;
pub(crate) mod inspection;
pub(crate) mod package;

pub use acquisition::{
    GitExecutableIdentity, GitNetworkTransferObservation, GitSourceRequest, GitSourceRequestError,
    GitSourceResolutionObservation, GitTransportExecutableIdentity, GitTransportProfile,
    LocalSourceLimits, LocalSourceResolutionObservation, ResolvedGitSource, ResolvedLocalSnapshot,
    ResolvedLocalSource, SourceResolveError, SourceResolverStorage,
    resolve_git_source_with_storage, resolve_local_source,
    resolve_local_source_snapshot_with_storage,
};
#[cfg(test)]
pub(crate) use closure::resolve_external_local_package_closure;
pub use closure::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, DependencyRequestPath, DependencyRequestPathStep,
    PackageClosureValidationError, PackageRootSourceRequest, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError, PackageSourceCustody,
    ResolveDependencySourceError, ResolveExternalLocalPackageClosureError,
    ResolveGitPackageClosureError, ResolveWorkspacePackageClosureError, ResolvedDependency,
    ResolvedDependencySourceRequest, ResolvedPackageClosure, ResolvedPackageNode,
    ResolvedPackageSourceClosure, ResolvedPackageSourceRequestSet,
    ResolvedRootPackageSourceRequest, ResolvedSourceIdentity,
    SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION, resolve_external_local_package_closure_with_storage,
    resolve_external_local_project_closure_with_storage, resolve_git_package_closure_with_storage,
    resolve_workspace_package_closure_in_context_with_storage,
    resolve_workspace_package_closure_with_storage,
};
pub use identity::{
    AliasName, ExternalLocalLineage, ExternalSourceContext, GenericGitLineage, GitCommitId,
    GitHubRepositoryLineage, GitLabRepositoryLineage, GitObjectIdAlgorithm, GitTransport,
    GitTreeId, IdentityError, ImmutableSourceResolution, PackageKey, PackageName,
    SourceContentDigest, SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberLineage,
    WorkspaceMemberPath,
};
pub use inspection::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, audit_package_source,
    audit_package_source_locator,
};
pub use package::{
    ResolvePackageSourceError, ResolvedPackageSource,
    resolve_external_local_package_source_with_storage,
    resolve_external_local_project_source_with_storage, resolve_git_package_source_with_storage,
    resolve_workspace_member_package_source_with_storage,
};
#[cfg(test)]
pub(crate) use package::{
    resolve_external_local_package_source, resolve_workspace_member_package_source,
};
