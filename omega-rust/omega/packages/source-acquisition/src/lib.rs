#![forbid(unsafe_code)]
#![cfg_attr(windows, feature(windows_by_handle))]

//! Package-source identity and hostile source acquisition.
//!
//! [`identity`] names immutable package sources, [`tree`] owns source-neutral
//! traversal and results, and [`local`] and [`git`] acquire hostile source
//! under shared snapshot and custody owners. This crate stops at immutable
//! source custody; package declarations, graph construction, review, and
//! admission belong to `package-manager`.

mod custody;
mod error;
pub mod git;
pub(crate) mod identity;
mod limits;
pub mod local;
mod observations;
mod snapshot;
pub(crate) mod source_resolver;
pub mod storage;
pub mod tree;

pub use error::SourceResolveError;
pub use git::request::{GitSourceRequest, GitSourceRequestError, GitTransportProfile};
pub use git::workspace::{
    GitWorkspaceDeclaration, GitWorkspaceDeclarationLimits,
    GitWorkspaceProjectionError, GitWorkspaceProjectionPlanner, GitWorkspaceProjectionResult,
    GitWorkspaceSelection,
};
pub use identity::{
    ExternalLocalLineage, ExternalSourceContext, GitCommitId, GitObjectIdAlgorithm, GitTransport,
    GitTreeId, IdentityError, ImmutableSourceResolution, SourceContentDigest, SourceLineage,
    SourceRelativePath, WorkspaceLineageIdentity, WorkspaceMemberLineage,
};
pub use limits::LocalSourceLimits;
pub use local::operations::{resolve_local_source};
pub use local::resolution_observations::{ ResolvedLocalSnapshot,
    ResolvedLocalSource,
};
pub use observations::resolved::{GitAcquisitionPin, ResolvedGitSource};
pub use source_resolver::{PrimaryGitChoices, SourceResolver};

#[cfg(test)]
mod test_support;

fn take_u64(bytes: &mut &[u8]) -> Option<u64> {
    let value = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
    *bytes = &bytes[8..];
    Some(value)
}

fn take_framed_bytes<'a>(bytes: &mut &'a [u8]) -> Option<&'a [u8]> {
    let length = usize::try_from(take_u64(bytes)?).ok()?;
    let value = bytes.get(..length)?;
    *bytes = &bytes[length..];
    Some(value)
}
