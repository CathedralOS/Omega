#![forbid(unsafe_code)]

//! Package-source identity and hostile source acquisition.
//!
//! [`identity`] names immutable package sources, [`local`] and [`git`] capture
//! hostile source under shared [`custody`]. This crate stops at immutable
//! source custody; package declarations, graph construction, review, and
//! admission belong to `omega-package-manager`.

mod custody;
mod error;
pub mod git;
pub(crate) mod identity;
mod limits;
pub mod local;
mod observations;
pub mod storage;

pub use error::SourceResolveError;
pub use git::request::{GitSourceRequest, GitSourceRequestError, GitTransportProfile};
pub use git::resolution::resolve_git_source_with_storage;
pub use git::resolution::resolve_git_workspace_member_with_storage;
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
#[cfg(test)]
pub(crate) use local::operations::resolve_local_source_snapshot_at_path;
pub use local::operations::{resolve_local_source, resolve_local_source_snapshot_with_storage};
pub use observations::accounting::GitNetworkTransferObservation;
pub use observations::execution::{
    GitCommandInputCommitment, GitExecutableIdentity, GitTransportExecutableIdentity,
};
pub use observations::receipt::{
    GitSourceStrictReceipt, GitSourceStrictReceiptError, GitSourceStrictReceiptRequirement,
};
pub use observations::resolution::GitSourceResolutionObservation;
pub use observations::resolved::{GitAcquisitionPin, ResolvedGitSource};
pub use storage::SourceResolverStorage;

#[cfg(test)]
mod tests;
