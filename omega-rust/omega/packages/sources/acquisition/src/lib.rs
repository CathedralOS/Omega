#![forbid(unsafe_code)]

//! Package-source identity and hostile source acquisition.
//!
//! [`identity`] names immutable package sources, [`tree`] owns source-neutral
//! traversal and results, and [`local`] and [`git`] acquire hostile source
//! under shared snapshot and custody owners. This crate stops at immutable
//! source custody; package declarations, graph construction, review, and
//! admission belong to `omega-package-manager`.

mod custody;
mod error;
pub mod git;
pub(crate) mod identity;
mod limits;
pub mod local;
mod observations;
mod snapshot;
pub mod storage;
pub mod tree;

pub use error::SourceResolveError;
pub use git::executable::selection::PrimaryGitSelection;
pub use git::request::{GitSourceRequest, GitSourceRequestError, GitTransportProfile};
pub use git::resolution::{resolve_git_source_with_primary_git, resolve_git_source_with_storage};
pub use git::resolution::{
    resolve_git_workspace_member_with_primary_git, resolve_git_workspace_member_with_storage,
};
pub use git::workspace::{
    GitWorkspaceDeclaration, GitWorkspaceDeclarationLimits, GitWorkspaceProjectionCustody,
    GitWorkspaceProjectionError, GitWorkspaceProjectionPlanner, GitWorkspaceProjectionResult,
    GitWorkspaceSelection,
};
pub use identity::{
    ExternalLocalLineage, ExternalSourceContext, GenericGitLineage, GitCommitId,
    GitHubRepositoryLineage, GitLabRepositoryLineage, GitObjectIdAlgorithm, GitTransport,
    GitTreeId, IdentityError, ImmutableSourceResolution, SourceContentDigest, SourceLineage,
    SourceRelativePath, WorkspaceLineageIdentity, WorkspaceMemberLineage,
};
pub use limits::LocalSourceLimits;
pub use local::model::{
    LocalSourceResolutionObservation, ResolvedLocalSnapshot, ResolvedLocalSource,
};
pub use local::operations::{resolve_local_source, resolve_local_source_snapshot_with_storage};
pub use observations::resolved::{GitAcquisitionPin, ResolvedGitSource};
pub use observations::storage::GitRetainedStorageCustody;
pub use storage::SourceResolverStorage;
pub use tree::ResolvedSourceTree;

#[cfg(test)]
mod test_support;
