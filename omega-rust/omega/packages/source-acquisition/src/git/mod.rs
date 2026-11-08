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
(`workspace`), we authenticate the root and member declaration blobs in memory,
delegate syntax-aware member selection to a `GitWorkspaceProjectionPlanner`,
and project only the selected member subtree into an isolated snapshot root.
*/

pub(crate) mod cache;
pub(crate) mod executable;
pub(crate) mod git_command;
pub(crate) mod objects;
pub mod request;
pub mod resolution;
pub(crate) mod snapshot;
pub(crate) mod snapshot_metadata;
pub mod workspace;

#[cfg(test)]
mod tests;
