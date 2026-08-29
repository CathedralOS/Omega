//! Authenticated Git tree materialization and immutable snapshot verification.

use crate::source::{
    CANONICAL_DIRECTORY_MODE, CacheCustodyKind, CapturedLocalEntry, CapturedLocalEntryKind,
    GIT_SNAPSHOT_METADATA, GIT_SNAPSHOT_SOURCE, GitBlobBytes, GitExecutor, GitTreeEntry,
    GitTreeEntryKind, LocalSourceLimits, ResolvedLocalSource, SourceIdentityHasher,
    SourceResolveError, SourceTreePolicy, VerifiedGitRepository, authenticate_git_tree,
    cache_invalid, capture_local_source_from_open_root, git_directory_paths, git_tree_invalid,
    io_error, open_absolute_directory_nofollow, read_bounded_cache_record,
    reconcile_git_cache_operation_result,
};
use cap_fs_ext::DirExt;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::construction::{
    create_snapshot_symlink_from_open_root, open_or_create_snapshot_directory,
    write_snapshot_file_from_open_root,
};
use super::metadata::{GitSnapshotMetadata, git_snapshot_metadata, parse_git_snapshot_metadata};
use super::permissions::{make_open_snapshot_read_only, verify_open_snapshot_tree_modes};
use super::publication::PendingMaterializedSnapshot;

pub(in crate::source) fn resolve_git_snapshot(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
    mut entries: Vec<GitTreeEntry>,
    limits: LocalSourceLimits,
) -> Result<(PathBuf, ResolvedLocalSource), SourceResolveError> {
    let expected = preflight_git_snapshot(tree, &entries)?;
    let snapshots = repository.open_or_create_snapshots()?;
    let publication = snapshots.path.join(format!("tree-{tree}"));
    if snapshots.publication_exists(&publication)? {
        release_git_blob_payloads(&mut entries);
        let result = verify_git_snapshot(&publication, &expected, &entries, limits);
        return reconcile_git_cache_operation_result(result, snapshots.verify_identity(), None);
    }

    let mut pending = PendingMaterializedSnapshot::create_from_open_parent(
        CacheCustodyKind::Git,
        &snapshots.path,
        &snapshots.directory,
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
                    CacheCustodyKind::Git,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                )?;
            }
            GitTreeEntryKind::File { executable, bytes } => {
                write_snapshot_file_from_open_root(
                    CacheCustodyKind::Git,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    bytes.as_slice(),
                    *executable,
                )?;
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                create_snapshot_symlink_from_open_root(
                    CacheCustodyKind::Git,
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
        CacheCustodyKind::Git,
        pending.directory()?,
        Path::new(GIT_SNAPSHOT_METADATA),
        &pending.root,
        &git_snapshot_metadata(tree, &staged),
        false,
    )?;
    make_open_snapshot_read_only(CacheCustodyKind::Git, pending.directory()?, &pending.root)?;
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
    pending.publish(&snapshots.path, &publication)?;

    // The returned identity is always calculated from the atomically published tree, never from
    // the staging directory or Git's mutable object-cache state.
    let result = verify_git_snapshot(&publication, &expected, &entries, limits);
    reconcile_git_cache_operation_result(result, snapshots.verify_identity(), None)
}

pub(in crate::source) fn preflight_git_snapshot(
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
    publication: &Path,
    expected: &GitSnapshotMetadata,
    entries: &[GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(PathBuf, ResolvedLocalSource), SourceResolveError> {
    let source = publication.join(GIT_SNAPSHOT_SOURCE);
    let metadata_path = publication.join(GIT_SNAPSHOT_METADATA);
    let metadata = read_bounded_cache_record(
        CacheCustodyKind::Git,
        publication,
        Path::new(GIT_SNAPSHOT_METADATA),
        1024,
    )?;
    let metadata = parse_git_snapshot_metadata(&metadata, &metadata_path)?;
    if metadata != *expected {
        return Err(cache_invalid(
            &metadata_path,
            "snapshot metadata does not match the authenticated Git tree",
        ));
    }
    let publication_directory = open_absolute_directory_nofollow(publication)
        .map_err(|error| cache_invalid(publication, error.to_string()))?;
    verify_open_snapshot_tree_modes(CacheCustodyKind::Git, &publication_directory, publication)?;
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
