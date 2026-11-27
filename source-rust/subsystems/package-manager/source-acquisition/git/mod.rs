/*
Git source request validation, sealed command execution, object-graph
authentication, and immutable snapshot publication.

We resolve remote Git repositories and workspace members without trusting the
ambient Git configuration, working tree checkout, or unauthenticated pack
state. Every Git resolution freezes one host-selected primary `git` executable
outside package-controlled roots (`executable`), runs bounded child commands
under a sealed environment and protocol allowlist (`git_command`), and caches
bare repositories in per-transport, per-locator custody slots (`cache`).
*/

/*
Why we authenticate Git objects in-process before materializing snapshots:

Git's local object store is a mutable cache, not an immutable package snapshot.
After fetching or opening a cached bare repository, `objects` reads the target
commit, tree, and blob objects in bounded batches, verifies their cryptographic
SHA-1/SHA-256 object digests and parent-child graph structure in-process, and
rejects symlinks that escape the root, gitlinks, or excluded paths. Only after
the complete tree is authenticated in memory does `snapshot` materialize a
read-only `tree-<object_id>` directory and re-verify the published bytes on
disk.
*/

/*
Provenance and workspace member projection:

`resolution` coordinates exact-revision and selector-based resolutions,
emitting `ResolvedGitSource` records paired with `GitResolutionObservation`
provenance. When a package resides inside a multi-package Git workspace
(`workspace`), a `GitWorkspaceSession` authenticates the root and requested
member declaration blobs in memory, leaves syntax-aware member selection to the
package manager, and projects only the selected member subtree into an isolated
snapshot root.
*/

pub(crate) mod cache;
pub(crate) mod executable;
pub mod execution;
pub(crate) mod git_command;
pub(crate) mod objects;
pub mod request;
pub mod resolution;
pub(crate) mod snapshot;
pub mod workspace;

#[cfg(test)]
mod tests;

use crate::error::cache_invalid;
use crate::git::objects::identity::is_object_id;
use crate::identity::digest::append_framed_bytes;
use crate::limits::GIT_SNAPSHOT_POLICY;
use crate::tree::ResolvedLocalSource;
use crate::{SourceResolveError, take_framed_bytes, take_u64};
use std::path::Path;

// Binary wire encoding and fail-closed parser (`GitSnapshotMetadata`) for
// authenticated Git snapshot metadata records (`GIT_SNAPSHOT_POLICY`).

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GitSnapshotMetadata {
    pub(super) tree: String,
    pub(super) file_count: usize,
    pub(super) byte_count: u64,
    pub(super) content_identity: String,
}

pub(crate) fn git_snapshot_metadata(tree: &str, local: &ResolvedLocalSource) -> Vec<u8> {
    let mut metadata = GIT_SNAPSHOT_POLICY.to_vec();
    append_framed_bytes(&mut metadata, tree.as_bytes());
    metadata.extend_from_slice(&(local.file_count as u64).to_le_bytes());
    metadata.extend_from_slice(&local.byte_count.to_le_bytes());
    append_framed_bytes(&mut metadata, local.content_identity.as_bytes());
    metadata
}

pub(crate) fn parse_git_snapshot_metadata(
    bytes: &[u8],
    path: &Path,
) -> Result<GitSnapshotMetadata, SourceResolveError> {
    let Some(mut remaining) = bytes.strip_prefix(GIT_SNAPSHOT_POLICY) else {
        return Err(cache_invalid(
            path,
            "snapshot metadata policy does not match",
        ));
    };
    let tree = take_framed_bytes(&mut remaining)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .filter(|tree| is_object_id(tree))
        .ok_or_else(|| cache_invalid(path, "snapshot metadata tree is invalid"))?
        .to_owned();
    let file_count = take_u64(&mut remaining)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| cache_invalid(path, "snapshot file count is invalid"))?;
    let byte_count = take_u64(&mut remaining)
        .ok_or_else(|| cache_invalid(path, "snapshot byte count is invalid"))?;
    let content_identity = take_framed_bytes(&mut remaining)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .filter(|identity| {
            identity.len() == 64 && identity.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .ok_or_else(|| cache_invalid(path, "snapshot content identity is invalid"))?
        .to_owned();
    if !remaining.is_empty() {
        return Err(cache_invalid(path, "snapshot metadata has trailing bytes"));
    }
    Ok(GitSnapshotMetadata {
        tree,
        file_count,
        byte_count,
        content_identity,
    })
}
