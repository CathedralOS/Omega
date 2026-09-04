//! Authenticated Git tree materialization and immutable snapshot verification.

use crate::SourceResolveError;
use crate::custody::lock::CacheEntryLock;
use crate::custody::tree::{CacheCustodyKind, cache_custody_invalid, read_bounded_cache_record};
use crate::error::{cache_invalid, git_tree_invalid};
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::commands::reconciliation::reconcile_git_cache_operation_result;
use crate::git::executable::executor::GitExecutor;
use crate::git::objects::authentication::authenticate_git_tree;
use crate::git::objects::tree::git_directory_paths;
use crate::git::objects::{GitBlobBytes, GitTreeEntry, GitTreeEntryKind};
use crate::limits::{
    CANONICAL_DIRECTORY_MODE, GIT_SNAPSHOT_METADATA, GIT_SNAPSHOT_SOURCE, LocalSourceLimits,
};
use crate::storage::RetainedStorageLane;
use crate::tree::ResolvedLocalSource;
use crate::tree::capture::{
    CapturedLocalEntry, CapturedLocalEntryKind, SourceTreePolicy,
    capture_local_source_from_open_root,
};
use crate::tree::filesystem::{io_error, open_absolute_directory_nofollow};
use crate::tree::identity::SourceIdentityHasher;
use cap_fs_ext::DirExt;
use cap_std::fs::Dir as CapabilityDirectory;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::snapshot_metadata::{
    GitSnapshotMetadata, git_snapshot_metadata, parse_git_snapshot_metadata,
};
use crate::snapshot::construction::{
    create_snapshot_symlink_from_open_root, open_or_create_snapshot_directory,
    write_snapshot_file_from_open_root,
};
use crate::snapshot::permissions::{make_open_snapshot_read_only, verify_open_snapshot_tree_modes};
use crate::snapshot::publication::PendingMaterializedSnapshot;

pub(crate) fn resolve_git_snapshot(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
    entries: Vec<GitTreeEntry>,
    limits: LocalSourceLimits,
) -> Result<(PathBuf, ResolvedLocalSource), SourceResolveError> {
    let expected = preflight_git_snapshot(tree, &entries)?;
    let snapshots = repository.open_or_create_snapshots()?;
    let publication = snapshots.path.join(format!("tree-{tree}"));
    let publication_exists = snapshots.publication_exists(&publication)?;
    let result = resolve_git_snapshot_in_collection(
        executor,
        CacheCustodyKind::Git,
        &snapshots.path,
        &snapshots.directory,
        tree,
        entries,
        &expected,
        limits,
        Some(publication_exists),
    );
    reconcile_git_cache_operation_result(result, snapshots.verify_identity(), None)
}

/// Publish an authenticated, re-rooted Git member tree directly beneath a
/// retained workspace-member lane.
///
/// The tree object ID is the publication key. Reuse never trusts that key
/// alone: metadata, custody, shape, modes, payload bytes, and reconstructed
/// source identity are verified again while the exact lane and entry lock are
/// retained.
pub(crate) fn publish_git_member_snapshot(
    executor: &GitExecutor,
    lane: &RetainedStorageLane,
    tree: &str,
    entries: Vec<GitTreeEntry>,
    limits: LocalSourceLimits,
) -> Result<(PathBuf, ResolvedLocalSource), SourceResolveError> {
    let expected = preflight_git_snapshot(tree, &entries)?;
    lane.verify_path_identity()?;

    let lock_name = format!("tree-{tree}.lock");
    let entry_lock = CacheEntryLock::acquire_local_from_parent(
        lane.path(),
        lane.directory(),
        OsStr::new(&lock_name),
    )?;
    lane.verify_path_identity()?;

    let result = resolve_git_snapshot_in_collection(
        executor,
        CacheCustodyKind::LocalSnapshot,
        lane.path(),
        lane.directory(),
        tree,
        entries,
        &expected,
        limits,
        None,
    );
    let retained_custody = entry_lock
        .verify_path_identity()
        .and_then(|()| lane.verify_path_identity());
    match retained_custody {
        Ok(()) => result,
        Err(error) => Err(error),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_git_snapshot_in_collection(
    executor: &GitExecutor,
    kind: CacheCustodyKind,
    collection_path: &Path,
    collection: &CapabilityDirectory,
    tree: &str,
    mut entries: Vec<GitTreeEntry>,
    expected: &GitSnapshotMetadata,
    limits: LocalSourceLimits,
    known_publication_exists: Option<bool>,
) -> Result<(PathBuf, ResolvedLocalSource), SourceResolveError> {
    let publication_name = format!("tree-{tree}");
    let publication = collection_path.join(&publication_name);
    let publication_exists = match known_publication_exists {
        Some(exists) => exists,
        None => snapshot_publication_exists(kind, collection, &publication_name, &publication)?,
    };
    if publication_exists {
        release_git_blob_payloads(&mut entries);
        return verify_git_snapshot(kind, &publication, expected, &entries, limits);
    }

    let mut pending = PendingMaterializedSnapshot::create_from_open_parent(
        kind,
        collection_path,
        collection,
        &format!(".tree-{tree}.stage"),
    )?;
    let source = pending.root.join(GIT_SNAPSHOT_SOURCE);
    pending
        .directory()?
        .create_dir(GIT_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;
    let source_directory = pending
        .directory()?
        .open_dir_nofollow(GIT_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;
    for entry in &entries {
        executor.verify_budget()?;
        checked_git_destination(&source, entry)?;
        match &entry.kind {
            GitTreeEntryKind::Tree => {
                open_or_create_snapshot_directory(
                    kind,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                )?;
            }
            GitTreeEntryKind::File { executable, bytes } => {
                write_snapshot_file_from_open_root(
                    kind,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    bytes.as_slice(),
                    *executable,
                )?;
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                create_snapshot_symlink_from_open_root(
                    kind,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    target_bytes.as_slice(),
                )?;
            }
        }
    }

    // The staged source is re-read to bind publication identity. Release the
    // shared batch payload first so that this verification does not retain a
    // second package-sized in-memory copy.
    release_git_blob_payloads(&mut entries);
    let staged = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if staged.file_count != expected.file_count
        || staged.byte_count != expected.byte_count
        || staged.content_identity != expected.content_identity
    {
        return Err(cache_invalid(
            &source,
            "materialized snapshot did not preserve the validated Git tree exactly",
        ));
    }
    write_snapshot_file_from_open_root(
        kind,
        pending.directory()?,
        Path::new(GIT_SNAPSHOT_METADATA),
        &pending.root,
        &git_snapshot_metadata(tree, &staged),
        false,
    )?;
    make_open_snapshot_read_only(kind, pending.directory()?, &pending.root)?;
    let finalized = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if finalized.file_count != expected.file_count
        || finalized.byte_count != expected.byte_count
        || finalized.content_identity != expected.content_identity
    {
        return Err(cache_invalid(
            &source,
            "finalized snapshot did not preserve the authenticated Git tree exactly",
        ));
    }
    // Windows will not rename the retained stage while a descendant directory
    // handle remains open. Verification is complete, so release it before the
    // atomic publication rename.
    drop(source_directory);
    pending.publish(collection_path, &publication)?;

    // The returned identity is always calculated from the atomically published tree, never from
    // the staging directory or Git's mutable object-cache state.
    verify_git_snapshot(kind, &publication, expected, &entries, limits)
}

fn snapshot_publication_exists(
    kind: CacheCustodyKind,
    collection: &CapabilityDirectory,
    publication_name: &str,
    publication: &Path,
) -> Result<bool, SourceResolveError> {
    match collection.symlink_metadata(publication_name) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(cache_custody_invalid(
                kind,
                publication,
                "Git snapshot publication is not a concrete directory",
            ))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(publication, error)),
    }
}

pub(crate) fn preflight_git_snapshot(
    tree: &str,
    entries: &[GitTreeEntry],
) -> Result<GitSnapshotMetadata, SourceResolveError> {
    authenticate_git_tree(tree, entries)?;
    verify_git_destination_containment(Path::new("omega-verified-snapshot-root"), entries)?;
    authenticated_git_snapshot_identity(tree, entries)
}

fn authenticated_git_snapshot_identity(
    tree: &str,
    entries: &[GitTreeEntry],
) -> Result<GitSnapshotMetadata, SourceResolveError> {
    let mut identity = SourceIdentityHasher::new(entries.len());
    let mut file_count = 0_usize;
    for entry in entries {
        match &entry.kind {
            GitTreeEntryKind::Tree => {
                identity.add_directory(&entry.relative_bytes, CANONICAL_DIRECTORY_MODE);
            }
            GitTreeEntryKind::File { executable, bytes } => {
                identity.add_file(&entry.relative_bytes, *executable, bytes.as_slice())?;
                file_count += 1;
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                identity.add_symlink(&entry.relative_bytes, target_bytes.as_slice());
                file_count += 1;
            }
        }
    }
    let (byte_count, content_identity) = identity.finish();
    Ok(GitSnapshotMetadata {
        tree: tree.to_owned(),
        file_count,
        byte_count,
        content_identity,
    })
}

fn verify_git_destination_containment(
    source: &Path,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    for entry in entries {
        checked_git_destination(source, entry)?;
    }
    Ok(())
}

fn checked_git_destination(
    source: &Path,
    entry: &GitTreeEntry,
) -> Result<PathBuf, SourceResolveError> {
    if entry
        .relative_path
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "materialization path is not strictly relative",
        ));
    }
    let destination = source.join(&entry.relative_path);
    if !destination.starts_with(source) {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "materialization path escapes the snapshot root",
        ));
    }
    Ok(destination)
}

fn release_git_blob_payloads(entries: &mut [GitTreeEntry]) {
    for entry in entries {
        match &mut entry.kind {
            GitTreeEntryKind::Tree => {}
            GitTreeEntryKind::File { bytes, .. } => *bytes = GitBlobBytes::empty(),
            GitTreeEntryKind::Symlink { target_bytes } => {
                *target_bytes = GitBlobBytes::empty();
            }
        }
    }
}

fn verify_git_snapshot(
    kind: CacheCustodyKind,
    publication: &Path,
    expected: &GitSnapshotMetadata,
    entries: &[GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(PathBuf, ResolvedLocalSource), SourceResolveError> {
    let source = publication.join(GIT_SNAPSHOT_SOURCE);
    let metadata_path = publication.join(GIT_SNAPSHOT_METADATA);
    let metadata =
        read_bounded_cache_record(kind, publication, Path::new(GIT_SNAPSHOT_METADATA), 1024)?;
    let metadata = parse_git_snapshot_metadata(&metadata, &metadata_path)?;
    if metadata != *expected {
        return Err(cache_invalid(
            &metadata_path,
            "snapshot metadata does not match the authenticated Git tree",
        ));
    }
    let publication_directory = open_absolute_directory_nofollow(publication)
        .map_err(|error| cache_invalid(publication, error.to_string()))?;
    verify_open_snapshot_tree_modes(kind, &publication_directory, publication)?;
    let source_directory = publication_directory
        .open_dir_nofollow(GIT_SNAPSHOT_SOURCE)
        .map_err(|error| cache_invalid(&source, error.to_string()))?;
    let captured = capture_local_source_from_open_root(
        source.clone(),
        source_directory,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?;
    verify_captured_git_snapshot_shape(&source, &captured.entries, entries)?;
    let local = captured.normalized;
    if local.file_count != expected.file_count
        || local.byte_count != expected.byte_count
        || local.content_identity != expected.content_identity
    {
        return Err(cache_invalid(
            publication,
            "published snapshot does not match the authenticated Git tree",
        ));
    }
    Ok((source, local))
}

fn verify_captured_git_snapshot_shape(
    source: &Path,
    captured: &[CapturedLocalEntry],
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    let mut expected_directories = git_directory_paths(entries);
    let mut expected_leaves = entries
        .iter()
        .filter(|entry| !matches!(entry.kind, GitTreeEntryKind::Tree))
        .map(|entry| (entry.relative_bytes.as_slice(), &entry.kind))
        .collect::<BTreeMap<_, _>>();
    for entry in captured {
        let path = source.join(&entry.relative_path);
        match &entry.kind {
            CapturedLocalEntryKind::Directory => {
                if !expected_directories.remove(&entry.relative_bytes) {
                    return Err(cache_invalid(
                        &path,
                        "snapshot contains an undeclared directory",
                    ));
                }
            }
            CapturedLocalEntryKind::File { executable, .. } => {
                let Some(expected) = expected_leaves.remove(entry.relative_bytes.as_slice()) else {
                    return Err(cache_invalid(&path, "snapshot contains an undeclared file"));
                };
                if !matches!(
                    expected,
                    GitTreeEntryKind::File {
                        executable: expected_executable,
                        ..
                    } if expected_executable == executable
                ) {
                    return Err(cache_invalid(
                        &path,
                        "snapshot file kind or executable mode does not match Git",
                    ));
                }
            }
            CapturedLocalEntryKind::Symlink { .. } => {
                let Some(expected) = expected_leaves.remove(entry.relative_bytes.as_slice()) else {
                    return Err(cache_invalid(
                        &path,
                        "snapshot contains an undeclared symlink",
                    ));
                };
                if !matches!(expected, GitTreeEntryKind::Symlink { .. }) {
                    return Err(cache_invalid(
                        &path,
                        "snapshot symlink kind does not match Git",
                    ));
                }
            }
        }
    }
    if !expected_directories.is_empty() || !expected_leaves.is_empty() {
        return Err(cache_invalid(
            source,
            "snapshot paths do not exactly match the validated Git tree",
        ));
    }
    Ok(())
}
