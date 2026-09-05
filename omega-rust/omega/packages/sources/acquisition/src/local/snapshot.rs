//! Local snapshot staging, publication, reuse, and topology checks.

mod materialize;
#[cfg(test)]
use materialize::materialize_local_snapshot;
use materialize::materialize_local_snapshot_from_open_parent;

use std::ffi::OsStr;
#[cfg(test)]
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::model::{ResolvedLocalSnapshot, ResolvedLocalSource};
use super::observation::issue_local_source_resolution_observation;
use super::operations::resolve_local_source;
use crate::SourceResolveError;
use crate::custody::lock::CacheEntryLock;
use crate::custody::tree::verify_local_cache_custody;
#[cfg(test)]
use crate::custody::tree::verify_local_cache_root_custody;
use crate::error::local_snapshot_invalid;
use crate::identity::digest::{format_sha256, hash_bytes};
use crate::limits::{LOCAL_CACHE_SNAPSHOTS, LOCAL_SNAPSHOT_CUSTODY_POLICY, LocalSourceLimits};
use crate::snapshot::metadata::verify_local_snapshot;
use crate::storage::RetainedStorageLane;
use crate::tree::capture::CapturedLocalTree;
use crate::tree::filesystem::{io_error, raw_os_bytes};

#[cfg(test)]
pub(crate) fn publish_local_snapshot(
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
    let entry_lock = CacheEntryLock::acquire_local(&lock_path)?;

    let normalized = if publication.exists() {
        let normalized = verify_local_snapshot(&publication, &identity, limits)?;
        verify_live_source_unchanged(&captured.normalized, limits)?;
        normalized
    } else {
        materialize_local_snapshot(&snapshots, &publication, &captured, limits)?
    };

    let normalized = finalize_local_tree(
        &requested_root,
        &captured.normalized,
        &captured.normalized,
        &publication,
        normalized,
        limits,
        || {
            entry_lock.verify_path_identity()?;
            verify_local_cache_root_custody(&canonical_cache_dir)?;
            verify_local_cache_root_custody(&snapshots)
        },
    )?;
    Ok(issue_local_snapshot(
        requested_root,
        &captured.normalized,
        &publication,
        normalized,
        limits,
    ))
}

pub(crate) fn publish_local_snapshot_in_lane(
    requested_root: PathBuf,
    captured: CapturedLocalTree,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    let normalized = publish_local_tree_in_lane(
        &requested_root,
        &captured,
        &captured.normalized,
        lane,
        limits,
    )?;
    let publication = normalized
        .root
        .parent()
        .expect("snapshot source has a publication parent")
        .to_path_buf();
    Ok(issue_local_snapshot(
        requested_root,
        &captured.normalized,
        &publication,
        normalized,
        limits,
    ))
}

pub(super) fn publish_local_tree_in_lane(
    requested_root: &Path,
    captured: &CapturedLocalTree,
    expected_live: &ResolvedLocalSource,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    lane.verify_path_identity()?;
    let result = (|| {
        validate_retained_local_snapshot_topology(&captured.normalized.root, lane.path())?;
        let snapshots = lane.retain_child(LOCAL_CACHE_SNAPSHOTS)?;
        publish_local_snapshot_in_retained_collection(
            requested_root,
            captured,
            expected_live,
            &snapshots,
            limits,
        )
    })();
    match lane.verify_path_identity() {
        Ok(()) => result,
        Err(error) => Err(error),
    }
}

fn publish_local_snapshot_in_retained_collection(
    requested_root: &Path,
    captured: &CapturedLocalTree,
    expected_live: &ResolvedLocalSource,
    snapshots: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
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
        let entry_lock = CacheEntryLock::acquire_local_from_parent(
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
            verify_live_source_unchanged(expected_live, limits)?;
            normalized
        } else {
            materialize_local_snapshot_from_open_parent(
                snapshots.path(),
                snapshots.directory(),
                &publication,
                captured,
                expected_live,
                limits,
            )?
        };
        finalize_local_tree(
            requested_root,
            expected_live,
            &captured.normalized,
            &publication,
            normalized,
            limits,
            || {
                entry_lock.verify_path_identity()?;
                snapshots.verify_path_identity()
            },
        )
    })();

    let snapshots_result = snapshots.verify_path_identity();
    match snapshots_result {
        Ok(()) => result,
        Err(error) => Err(error),
    }
}

fn finalize_local_tree(
    requested_root: &Path,
    captured_live_source: &ResolvedLocalSource,
    proposed_source: &ResolvedLocalSource,
    publication: &Path,
    expected_snapshot: ResolvedLocalSource,
    limits: LocalSourceLimits,
    verify_outer_custody: impl Fn() -> Result<(), SourceResolveError>,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let limits = limits.compiler_bounded();
    verify_outer_custody()?;
    verify_requested_local_root(requested_root, &captured_live_source.root)?;
    verify_local_cache_custody(publication, limits)?;

    let final_snapshot =
        verify_local_snapshot(publication, &proposed_source.content_identity, limits)?;
    if final_snapshot.root != expected_snapshot.root
        || !same_source_identity(&final_snapshot, &expected_snapshot)
        || !same_source_identity(&final_snapshot, proposed_source)
    {
        return Err(local_snapshot_invalid(
            publication,
            "final local snapshot rehash diverged before result issuance",
        ));
    }

    verify_live_source_unchanged(captured_live_source, limits)?;
    verify_local_cache_custody(publication, limits)?;
    verify_outer_custody()?;
    verify_requested_local_root(requested_root, &captured_live_source.root)?;
    Ok(final_snapshot)
}

fn issue_local_snapshot(
    requested_root: PathBuf,
    captured_live_source: &ResolvedLocalSource,
    publication: &Path,
    final_snapshot: ResolvedLocalSource,
    limits: LocalSourceLimits,
) -> ResolvedLocalSnapshot {
    let observation = issue_local_source_resolution_observation(
        &requested_root,
        &captured_live_source.root,
        publication,
        &final_snapshot,
        limits,
    );
    ResolvedLocalSnapshot::from_issued_parts(
        requested_root,
        captured_live_source.root.clone(),
        final_snapshot.root.clone(),
        final_snapshot,
        observation,
    )
}

pub(super) fn verify_requested_local_root(
    requested_root: &Path,
    expected_canonical_root: &Path,
) -> Result<(), SourceResolveError> {
    let canonical =
        requested_root
            .canonicalize()
            .map_err(|_| SourceResolveError::LocalSourceChanged {
                path: requested_root.to_path_buf(),
            })?;
    if canonical != expected_canonical_root {
        return Err(SourceResolveError::LocalSourceChanged {
            path: requested_root.to_path_buf(),
        });
    }
    Ok(())
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

pub(crate) fn local_snapshot_custody_identity(
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

pub(super) fn verify_live_source_unchanged(
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
