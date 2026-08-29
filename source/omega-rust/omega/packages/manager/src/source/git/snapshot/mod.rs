//! Authenticated immutable source snapshots.
//!
//! Git tree authentication and materialization enter through
//! [`resolve_git_snapshot`]. Shared snapshot construction, metadata,
//! permission finalization, and atomic publication live in focused sibling
//! modules because local-source snapshots use the same custody machinery.

mod construction;
mod materialization;
mod metadata;
mod permissions;
mod publication;

pub(in crate::source) use construction::{
    create_snapshot_symlink_from_open_root, open_or_create_snapshot_directory,
    write_snapshot_file_from_open_root,
};
#[cfg(test)]
pub(in crate::source) use materialization::preflight_git_snapshot;
pub(in crate::source) use materialization::resolve_git_snapshot;
#[cfg(test)]
pub(in crate::source) use metadata::git_snapshot_metadata;
// Retain the former module surface for source-resolver tests that name the
// metadata type directly, even though current tests only infer it.
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::source) use metadata::GitSnapshotMetadata;
pub(in crate::source) use metadata::{local_snapshot_metadata, verify_local_snapshot};
pub(in crate::source) use permissions::{
    make_open_snapshot_read_only, make_open_tree_owner_writable, verify_open_snapshot_tree_modes,
};
#[cfg(test)]
pub(in crate::source) use permissions::{make_snapshot_read_only, make_tree_owner_writable};
pub(in crate::source) use publication::PendingMaterializedSnapshot;
