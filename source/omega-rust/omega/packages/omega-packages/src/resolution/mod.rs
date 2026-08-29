//! Source identity, hostile acquisition, and complete closure reconciliation.

pub(crate) mod closure_resolution;
pub(crate) mod graph;
pub(crate) mod identity;
pub(crate) mod package_source;
pub(crate) mod source;
pub(crate) mod source_adapter;
pub(crate) mod source_closure_subject;
pub(crate) mod source_commands;

pub use closure_resolution::{
    DependencyRequestPath, DependencyRequestPathStep, PackageRootSourceRequest,
    PackageSourceClosureConflict, PackageSourceClosureConflictCandidate,
    PackageSourceClosureLimitKind, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    PackageSourceCustody, ResolvedDependencySourceRequest, ResolvedPackageSourceClosure,
    ResolvedPackageSourceRequestSet, ResolvedRootPackageSourceRequest,
};
pub use graph::{
    PackageClosureValidationError, ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode,
    ResolvedSourceIdentity,
};
pub use identity::{
    AliasName, ExternalLocalLineage, ExternalSourceContext, GenericGitLineage, GitCommitId,
    GitHubRepositoryLineage, GitLabRepositoryLineage, GitObjectIdAlgorithm, GitTransport,
    GitTreeId, IdentityError, ImmutableSourceResolution, PackageKey, PackageName,
    SourceContentDigest, SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberLineage,
    WorkspaceMemberPath,
};
pub use package_source::{
    ResolvePackageSourceError, ResolvedPackageSource, resolve_external_local_package_source,
    resolve_external_local_project_source, resolve_git_package_source,
    resolve_workspace_member_package_source,
};
pub use source::{
    GitExecutableIdentity, GitNetworkTransferObservation, GitSourceRequest, GitSourceRequestError,
    GitSourceResolutionObservation, GitTransportExecutableIdentity, GitTransportProfile,
    LocalSourceLimits, ResolvedGitSource, ResolvedLocalSnapshot, ResolvedLocalSource,
    SourceResolveError, resolve_git_source, resolve_local_source, resolve_local_source_snapshot,
};
pub use source_adapter::{
    ResolveDependencySourceError, ResolveExternalLocalPackageClosureError,
    ResolveGitPackageClosureError, ResolveWorkspacePackageClosureError,
    resolve_external_local_package_closure, resolve_external_local_project_closure,
    resolve_git_package_closure, resolve_workspace_package_closure,
    resolve_workspace_package_closure_in_context,
};
pub use source_closure_subject::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
};
pub use source_commands::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, audit_package_source,
    audit_package_source_locator,
};
