//! Source identity, hostile acquisition, and complete closure reconciliation.

pub(crate) mod identity;
#[path = "closure/mod.rs"]
pub mod package_closure;
pub(crate) mod package_source;
pub(crate) mod source;
pub(crate) mod source_commands;

pub use identity::{
    AliasName, ExternalLocalLineage, ExternalSourceContext, GenericGitLineage, GitCommitId,
    GitHubRepositoryLineage, GitLabRepositoryLineage, GitObjectIdAlgorithm, GitTransport,
    GitTreeId, IdentityError, ImmutableSourceResolution, PackageKey, PackageName,
    SourceContentDigest, SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberLineage,
    WorkspaceMemberPath,
};
pub use package_closure::*;
pub use package_source::{
    ResolvePackageSourceError, ResolvedPackageSource, resolve_external_local_package_source,
    resolve_external_local_project_source, resolve_git_package_source,
    resolve_workspace_member_package_source,
};
pub use source::{
    GitExecutableIdentity, GitNetworkTransferObservation, GitSourceRequest, GitSourceRequestError,
    GitSourceResolutionObservation, GitTransportExecutableIdentity, GitTransportProfile,
    LocalSourceLimits, ResolvedGitSource, ResolvedLocalSnapshot, ResolvedLocalSource,
    SourceResolveError, SourceResolverStorage, resolve_git_source, resolve_local_source,
    resolve_local_source_snapshot,
};
pub use source_commands::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, audit_package_source,
    audit_package_source_locator,
};
