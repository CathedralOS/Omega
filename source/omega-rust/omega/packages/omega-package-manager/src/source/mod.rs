//! Package source identity, request resolution, and hostile acquisition.
//!
//! Read this module from the outside inward:
//! [`identity`] names immutable package sources, [`resolution`] connects one
//! declared package request to a retained source, [`acquisition`] captures and
//! authenticates hostile local or Git content, and [`audit`] exposes the
//! source-inspection command boundary.

pub(crate) mod acquisition;
pub(crate) mod audit;
pub(crate) mod identity;
pub(crate) mod resolution;

pub use acquisition::{
    GitExecutableIdentity, GitNetworkTransferObservation, GitSourceRequest, GitSourceRequestError,
    GitSourceResolutionObservation, GitTransportExecutableIdentity, GitTransportProfile,
    LocalSourceLimits, ResolvedGitSource, ResolvedLocalSnapshot, ResolvedLocalSource,
    SourceResolveError, SourceResolverStorage, resolve_git_source_with_storage,
    resolve_local_source, resolve_local_source_snapshot_with_storage,
};
pub use audit::{
    PackageSourceAudit, PackageSourceAuditCommandError, PackageSourceRequest,
    PackageSourceRequestParseError, SourceAdapter, audit_package_source,
    audit_package_source_locator,
};
pub use identity::{
    AliasName, ExternalLocalLineage, ExternalSourceContext, GenericGitLineage, GitCommitId,
    GitHubRepositoryLineage, GitLabRepositoryLineage, GitObjectIdAlgorithm, GitTransport,
    GitTreeId, IdentityError, ImmutableSourceResolution, PackageKey, PackageName,
    SourceContentDigest, SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberLineage,
    WorkspaceMemberPath,
};
pub use resolution::{
    ResolvePackageSourceError, ResolvedPackageSource,
    resolve_external_local_package_source_with_storage,
    resolve_external_local_project_source_with_storage, resolve_git_package_source_with_storage,
    resolve_workspace_member_package_source_with_storage,
};
#[cfg(test)]
pub(crate) use resolution::{
    resolve_external_local_package_source, resolve_workspace_member_package_source,
};
