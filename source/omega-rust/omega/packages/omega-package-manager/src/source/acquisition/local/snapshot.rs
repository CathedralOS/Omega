//! Local snapshot staging, publication, reuse, and topology checks.

use std::ffi::OsStr;
#[cfg(test)]
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use cap_fs_ext::DirExt;
use sha2::{Digest, Sha256};

use super::capture::{
    CapturedLocalEntryKind, CapturedLocalTree, SourceTreePolicy,
    capture_local_source_from_open_root, hash_bytes, io_error, raw_os_bytes,
};
use super::model::{ResolvedLocalSnapshot, ResolvedLocalSource};
use super::operations::resolve_local_source;
#[cfg(test)]
use crate::source::acquisition::custody::verify_local_cache_root_custody;
use crate::source::acquisition::custody::{
    CacheCustodyKind, CacheEntryLock, verify_local_cache_custody,
};
use crate::source::acquisition::git::execution::format_sha256;
use crate::source::acquisition::git::{
    PendingMaterializedSnapshot, create_snapshot_symlink_from_open_root, local_snapshot_invalid,
    local_snapshot_metadata, make_open_snapshot_read_only, open_or_create_snapshot_directory,
    verify_local_snapshot, write_snapshot_file_from_open_root,
};
use crate::source::acquisition::storage::RetainedStorageLane;
use crate::source::acquisition::{
    LOCAL_CACHE_SNAPSHOTS, LOCAL_SNAPSHOT_CUSTODY_POLICY, LOCAL_SNAPSHOT_METADATA,
    LOCAL_SNAPSHOT_SOURCE, LocalSourceLimits, SourceResolveError,
};

#[cfg(test)]
pub(in crate::source::acquisition) fn publish_local_snapshot(
    requested_root: PathBuf,
    captured: CapturedLocalTree,
    cache_dir: &Path,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    let canonical_cache_dir =
        validate_local_snapshot_topology(&captured.normalized.root, cache_dir)?;
    std::fs::create_dir_all(&canonical_cache_dir)
        .map_err(|error| io_error(&canonical_cache_dir, error))?;
    let snapshots = canonical_cache_dir.join(LOCAL_CACHE_SNAPSHOTS);
    std::fs::create_dir_all(&snapshots).map_err(|error| io_error(&snapshots, error))?;
    verify_local_cache_root_custody(&canonical_cache_dir)?;
    verify_local_cache_root_custody(&snapshots)?;

    let identity = captured.normalized.content_identity.clone();
    let custody_identity = local_snapshot_custody_identity(
        &captured.normalized.root,
        &captured.normalized.content_identity,
    );
    let publication = snapshots.join(format!("source-{custody_identity}"));
    let lock_path = snapshots.join(format!("source-{custody_identity}.lock"));
    let _entry_lock = CacheEntryLock::acquire_local(&lock_path)?;

    let normalized = if publication.exists() {
        let normalized = verify_local_snapshot(&publication, &identity, limits)?;
        verify_live_source_unchanged(&captured.normalized, limits)?;
        normalized
    } else {
        materialize_local_snapshot(&snapshots, &publication, &captured, limits)?
    };

    verify_local_cache_root_custody(&canonical_cache_dir)?;
    verify_local_cache_root_custody(&snapshots)?;
    verify_local_cache_custody(&publication, limits)?;
    Ok(ResolvedLocalSnapshot {
        requested_root,
        canonical_live_root: captured.normalized.root,
        snapshot_root: normalized.root.clone(),
        normalized,
    })
}

pub(crate) fn publish_local_snapshot_in_lane(
    requested_root: PathBuf,
    captured: CapturedLocalTree,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    lane.verify_path_identity()?;
    let result = (|| {
        validate_retained_local_snapshot_topology(&captured.normalized.root, lane.path())?;
        let snapshots = lane.retain_child(LOCAL_CACHE_SNAPSHOTS)?;
        publish_local_snapshot_in_retained_collection(requested_root, captured, &snapshots, limits)
    })();
    match lane.verify_path_identity() {
        Ok(()) => result,
        Err(error) => Err(error),
    }
}

fn publish_local_snapshot_in_retained_collection(
    requested_root: PathBuf,
    captured: CapturedLocalTree,
    snapshots: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    snapshots.verify_path_identity()?;
    let identity = captured.normalized.content_identity.clone();
    let custody_identity = local_snapshot_custody_identity(
        &captured.normalized.root,
        &captured.normalized.content_identity,
    );
    let publication_name = format!("source-{custody_identity}");
    let publication = snapshots.path().join(&publication_name);
    let lock_name = format!("source-{custody_identity}.lock");
    let result = (|| {
        let _entry_lock = CacheEntryLock::acquire_local_from_parent(
            snapshots.path(),
            snapshots.directory(),
            OsStr::new(&lock_name),
        )?;

        let publication_exists = match snapshots.directory().symlink_metadata(&publication_name) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(local_snapshot_invalid(
                    &publication,
                    "local snapshot publication is not a concrete directory",
                ));
            }
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(io_error(&publication, error)),
        };
        let normalized = if publication_exists {
            let normalized = verify_local_snapshot(&publication, &identity, limits)?;
            verify_live_source_unchanged(&captured.normalized, limits)?;
            normalized
        } else {
            materialize_local_snapshot_from_open_parent(
                snapshots.path(),
                snapshots.directory(),
                &publication,
                &captured,
                limits,
            )?
        };

        verify_local_cache_custody(&publication, limits)?;
        Ok(ResolvedLocalSnapshot {
            requested_root,
            canonical_live_root: captured.normalized.root,
            snapshot_root: normalized.root.clone(),
            normalized,
        })
    })();

    let snapshots_result = snapshots.verify_path_identity();
    match snapshots_result {
        Ok(()) => result,
        Err(error) => Err(error),
    }
}

#[cfg(test)]
fn validate_local_snapshot_topology(
    canonical_live_root: &Path,
    cache_dir: &Path,
) -> Result<PathBuf, SourceResolveError> {
    let canonical_cache_dir = canonicalize_prospective_path(cache_dir)?;
    let snapshot_collection =
        canonicalize_prospective_path(&canonical_cache_dir.join(LOCAL_CACHE_SNAPSHOTS))?;
    if canonical_cache_dir.starts_with(canonical_live_root)
        || canonical_live_root.starts_with(&snapshot_collection)
    {
        return Err(SourceResolveError::LocalSnapshotCacheOverlapsSource {
            canonical_live_root: canonical_live_root.to_path_buf(),
            canonical_cache_dir,
        });
    }
    Ok(canonical_cache_dir)
}

fn validate_retained_local_snapshot_topology(
    canonical_live_root: &Path,
    canonical_cache_dir: &Path,
) -> Result<(), SourceResolveError> {
    let snapshot_collection = canonical_cache_dir.join(LOCAL_CACHE_SNAPSHOTS);
    if canonical_cache_dir.starts_with(canonical_live_root)
        || canonical_live_root.starts_with(&snapshot_collection)
    {
        return Err(SourceResolveError::LocalSnapshotCacheOverlapsSource {
            canonical_live_root: canonical_live_root.to_path_buf(),
            canonical_cache_dir: canonical_cache_dir.to_path_buf(),
        });
    }
    Ok(())
}

pub(in crate::source::acquisition) fn local_snapshot_custody_identity(
    canonical_live_root: &Path,
    content_identity: &str,
) -> String {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, LOCAL_SNAPSHOT_CUSTODY_POLICY);
    hash_bytes(
        &mut hasher,
        raw_os_bytes(canonical_live_root.as_os_str()).as_slice(),
    );
    hash_bytes(&mut hasher, content_identity.as_bytes());
    format_sha256(&hasher.finalize())
}

#[cfg(test)]
fn canonicalize_prospective_path(path: &Path) -> Result<PathBuf, SourceResolveError> {
    let mut existing = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| io_error(Path::new("."), error))?
            .join(path)
    };
    let mut suffix = Vec::<OsString>::new();
    loop {
        match std::fs::symlink_metadata(&existing) {
            Ok(_) => {
                let canonical = existing
                    .canonicalize()
                    .map_err(|error| io_error(&existing, error))?;
                let mut result = canonical;
                for component in suffix.into_iter().rev() {
                    if component == "." {
                        continue;
                    }
                    if component == ".." {
                        result.pop();
                    } else {
                        result.push(component);
                    }
                }
                return Ok(result);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(component) = existing.file_name().map(OsStr::to_os_string) else {
                    return Err(io_error(&existing, error));
                };
                suffix.push(component);
                existing.pop();
            }
            Err(error) => return Err(io_error(&existing, error)),
        }
    }
}

#[cfg(test)]
fn materialize_local_snapshot(
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
    materialize_pending_local_snapshot(pending, snapshots, publication, captured, limits)
}

fn materialize_local_snapshot_from_open_parent(
    snapshots: &Path,
    retained_snapshots: &cap_std::fs::Dir,
    publication: &Path,
    captured: &CapturedLocalTree,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let identity = &captured.normalized.content_identity;
    let pending = PendingMaterializedSnapshot::create_from_open_parent(
        CacheCustodyKind::LocalSnapshot,
        snapshots,
        retained_snapshots,
        &format!(".source-{identity}.stage"),
    )?;
    materialize_pending_local_snapshot(pending, snapshots, publication, captured, limits)
}

fn materialize_pending_local_snapshot(
    mut pending: PendingMaterializedSnapshot,
    snapshots: &Path,
    publication: &Path,
    captured: &CapturedLocalTree,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let identity = &captured.normalized.content_identity;
    let source = pending.root.join(LOCAL_SNAPSHOT_SOURCE);
    pending
        .directory()?
        .create_dir(LOCAL_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;
    let source_directory = pending
        .directory()?
        .open_dir_nofollow(LOCAL_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;

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

    let staged = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if !same_source_identity(&staged, &captured.normalized) {
        return Err(local_snapshot_invalid(
            &source,
            "staged source does not match the captured local tree",
        ));
    }
    verify_live_source_unchanged(&captured.normalized, limits)?;

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
    let finalized = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if !same_source_identity(&finalized, &captured.normalized) {
        return Err(local_snapshot_invalid(
            &source,
            "finalized snapshot does not match the captured local tree",
        ));
    }
    pending.publish(snapshots, publication)?;
    verify_local_snapshot(publication, identity, limits)
}

fn verify_live_source_unchanged(
    captured: &ResolvedLocalSource,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    let current = resolve_local_source(&captured.root, limits).map_err(|_| {
        SourceResolveError::LocalSourceChanged {
            path: captured.root.clone(),
        }
    })?;
    if !same_source_identity(&current, captured) {
        return Err(SourceResolveError::LocalSourceChanged {
            path: captured.root.clone(),
        });
    }
    Ok(())
}

fn same_source_identity(left: &ResolvedLocalSource, right: &ResolvedLocalSource) -> bool {
    left.file_count == right.file_count
        && left.byte_count == right.byte_count
        && left.content_identity == right.content_identity
}
