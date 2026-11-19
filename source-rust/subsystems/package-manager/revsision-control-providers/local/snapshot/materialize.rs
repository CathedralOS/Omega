// Materialization of captured local-source bytes into a read-only published
// snapshot directory.
//
// `materialize_pending_local_snapshot` writes every captured directory, file,
// and symlink into a staging directory, re-captures the materialized tree to
// confirm its identity matches the captured tree, and performs one final
// `verify_live_source_unchanged` check before writing metadata so a concurrent
// edit to the live checkout never leaves a durable snapshot of a transient tree
// state. On Windows, we explicitly drop the open `source_directory` handle
// before `pending.publish` so the OS permits the atomic directory rename.

use super::ResolvedLocalSource;
use super::{same_source_identity, verify_selected_live_source_unchanged};
use crate::SourceResolveError;
use crate::custody::tree::CacheCustodyKind;
use crate::error::local_snapshot_invalid;
use crate::limits::{LOCAL_SNAPSHOT_METADATA, LOCAL_SNAPSHOT_SOURCE, LocalSourceLimits};
use crate::snapshot::construction::{
    create_snapshot_symlink_from_open_root, open_or_create_snapshot_directory,
    write_snapshot_file_from_open_root,
};
use crate::snapshot::metadata::{local_snapshot_metadata, verify_local_snapshot};
use crate::snapshot::permissions::make_open_snapshot_read_only;
use crate::snapshot::publication::PendingMaterializedSnapshot;
use crate::tree::capture::{CapturedLocalEntryKind, CapturedLocalTree, capture_exact_materialized};
use crate::tree::filesystem::IoResultAt;
use cap_fs_ext::DirExt;
use std::path::Path;

#[cfg(test)]
pub(super) fn materialize_local_snapshot(
    snapshots: &Path,
    publication: &Path,
    captured: &CapturedLocalTree,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let identity = &captured.normalized.content_identity;
    let pending = PendingMaterializedSnapshot::create(
        CacheCustodyKind::LocalSnapshot,
        snapshots,
        &format!(".source-{identity}.stage"),
    )?;
    materialize_pending_local_snapshot(
        pending,
        snapshots,
        publication,
        captured,
        &captured.normalized,
        limits,
    )
}

pub(super) fn materialize_local_snapshot_from_open_parent(
    snapshots: &Path,
    retained_snapshots: &cap_std::fs::Dir,
    publication: &Path,
    captured: &CapturedLocalTree,
    expected_live: &ResolvedLocalSource,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let identity = &captured.normalized.content_identity;
    let pending = PendingMaterializedSnapshot::create_from_open_parent(
        CacheCustodyKind::LocalSnapshot,
        snapshots,
        retained_snapshots,
        &format!(".source-{identity}.stage"),
    )?;
    materialize_pending_local_snapshot(
        pending,
        snapshots,
        publication,
        captured,
        expected_live,
        limits,
    )
}

fn materialize_pending_local_snapshot(
    mut pending: PendingMaterializedSnapshot,
    snapshots: &Path,
    publication: &Path,
    captured: &CapturedLocalTree,
    expected_live: &ResolvedLocalSource,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let identity = &captured.normalized.content_identity;
    let source = pending.root.join(LOCAL_SNAPSHOT_SOURCE);
    pending
        .directory()?
        .create_dir(LOCAL_SNAPSHOT_SOURCE)
        .at(&source)?;
    let source_directory = pending
        .directory()?
        .open_dir_nofollow(LOCAL_SNAPSHOT_SOURCE)
        .at(&source)?;

    for entry in &captured.entries {
        match &entry.kind {
            CapturedLocalEntryKind::Directory => {
                open_or_create_snapshot_directory(
                    CacheCustodyKind::LocalSnapshot,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                )?;
            }
            CapturedLocalEntryKind::File { bytes, executable } => {
                write_snapshot_file_from_open_root(
                    CacheCustodyKind::LocalSnapshot,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    bytes,
                    *executable,
                )?;
            }
            CapturedLocalEntryKind::Symlink { target_bytes } => {
                create_snapshot_symlink_from_open_root(
                    CacheCustodyKind::LocalSnapshot,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    target_bytes,
                )?;
            }
        }
    }

    let staged = capture_exact_materialized(&source, &source_directory, limits)?.normalized;
    if !same_source_identity(&staged, &captured.normalized) {
        return Err(local_snapshot_invalid(
            &source,
            "staged source does not match the captured local tree",
        ));
    }
    verify_selected_live_source_unchanged(expected_live, limits, &captured.selected)?;

    write_snapshot_file_from_open_root(
        CacheCustodyKind::LocalSnapshot,
        pending.directory()?,
        Path::new(LOCAL_SNAPSHOT_METADATA),
        &pending.root,
        &local_snapshot_metadata(&staged),
        false,
    )?;
    make_open_snapshot_read_only(
        CacheCustodyKind::LocalSnapshot,
        pending.directory()?,
        &pending.root,
    )?;
    drop(source_directory);
    pending.publish(snapshots, publication)?;
    verify_local_snapshot(publication, identity, limits)
}
