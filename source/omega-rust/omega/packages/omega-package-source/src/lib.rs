#![forbid(unsafe_code)]

//! Package-source identity and hostile source acquisition.
//!
//! [`identity`] names immutable package sources, [`local`] and [`git`] capture
//! hostile source under shared [`custody`]. This crate stops at immutable
//! source custody; package declarations, graph construction, review, and
//! admission belong to `omega-package-manager`.

mod custody;
mod error;
mod git;
pub(crate) mod identity;
mod limits;
mod local;
mod observations;
mod storage;

pub use error::SourceResolveError;
pub use git::request::{GitSourceRequest, GitSourceRequestError, GitTransportProfile};
#[doc(hidden)]
pub use git::resolve::resolve_git_source_in_lane;
pub use git::resolve::resolve_git_source_with_storage;
pub use identity::{
    AliasName, ExternalLocalLineage, ExternalSourceContext, GenericGitLineage, GitCommitId,
    GitHubRepositoryLineage, GitLabRepositoryLineage, GitObjectIdAlgorithm, GitTransport,
    GitTreeId, IdentityError, ImmutableSourceResolution, PackageKey, PackageName,
    SourceContentDigest, SourceLineage, WorkspaceLineageIdentity, WorkspaceMemberLineage,
    WorkspaceMemberPath,
};
pub use limits::LocalSourceLimits;
pub use local::model::{
    LocalSourceResolutionObservation, ResolvedLocalSnapshot, ResolvedLocalSource,
};
#[doc(hidden)]
pub use local::model::{VerifiedPackageSourceEntry, VerifiedPackageSourceEntryKind};
#[cfg(test)]
pub(crate) use local::operations::resolve_local_source_snapshot_at_path;
#[doc(hidden)]
pub use local::operations::resolve_local_source_snapshot_in_lane;
#[doc(hidden)]
pub use local::operations::{
    capture_verified_package_source_snapshot, verify_package_source_snapshot,
};
pub use local::operations::{resolve_local_source, resolve_local_source_snapshot_with_storage};
pub use observations::accounting::GitNetworkTransferObservation;
pub use observations::execution::{
    GitCommandInputObservation, GitExecutableIdentity, GitTransportExecutableIdentity,
};
pub use observations::receipt::{
    GitSourceStrictReceipt, GitSourceStrictReceiptError, GitSourceStrictReceiptRequirement,
};
pub use observations::resolution::GitSourceResolutionObservation;
pub use observations::resolved::ResolvedGitSource;
#[doc(hidden)]
pub use storage::RetainedStorageLane;
pub use storage::SourceResolverStorage;

#[cfg(test)]
mod tests;
