/*
We establish package-source identity and acquire hostile local and Git sources
into immutable, capability-custodied snapshots.

`identity` names immutable package sources (`SourceLineage`,
`ImmutableSourceResolution`, and canonical digests), `tree` owns source-neutral
capability-relative traversal and content hashing, and `local` and `git` acquire
hostile source trees under shared `snapshot` and `custody` machinery. We stop at
immutable source custody; package declarations, dependency graph construction,
review, and admission belong to `package-manager`.
*/

/*
Hostile source trees on a developer machine or remote Git transport can mutate
mid-read, traverse symlinks outside the package root, or tamper with shared cache
directories. By routing every acquisition through capability-rooted directory
handles, bounded process execution, and read-only published snapshots verified
against domain-separated digests, we ensure downstream compilation and review
consume a fixed byte-for-byte source tree.
*/

/*
- Capability-rooted directory custody: `custody`, `storage`, `source_resolver`
- Source lineage and object digests: `identity`
- Bounded tree capture and hashing: `tree`, `snapshot`
- Local and Git acquisition pipelines: `local`, `git`, `observations`
*/

#![forbid(unsafe_code)]
#![cfg_attr(windows, feature(windows_by_handle))]

mod custody;
pub(crate) use optimization_core::digest_encoder;
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
pub use filesystem_authority::{SourceCaptureObligation, SourceCaptureRequest};
pub use git::request::{GitSourceRequest, GitSourceRequestError, GitTransportProfile};
pub use git::workspace::{
    GitWorkspaceDeclaration, GitWorkspaceDeclarationLimits, GitWorkspaceProjectionError,
    GitWorkspaceProjectionPlanner, GitWorkspaceProjectionResult, GitWorkspaceSelection,
};
pub use identity::{
    ExternalLocalLineage, ExternalSourceContext, GitCommitId, GitObjectIdAlgorithm, GitTransport,
    GitTreeId, IdentityError, ImmutableSourceResolution, SourceContentDigest, SourceLineage,
    SourceRelativePath, WorkspaceLineageIdentity, WorkspaceMemberLineage,
};
pub use limits::LocalSourceLimits;
pub use local::operations::resolve_local_source;
pub use local::resolution_observations::{ResolvedLocalSnapshot, ResolvedLocalSource};
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
