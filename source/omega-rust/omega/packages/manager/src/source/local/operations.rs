//! Public local resolution and package-snapshot verification operations.

use std::path::Path;

use crate::source::SourceResolveError;
use crate::source::custody::tree::CacheCustodyKind;
use crate::source::git::cache::identity::local_snapshot_invalid;
use crate::source::git::snapshot::permissions::verify_open_snapshot_tree_modes;
use crate::source::identity::SourceContentDigest;
use crate::source::limits::LocalSourceLimits;
use crate::source::storage::RetainedStorageLane;
use crate::source::storage::SourceResolverStorage;

use super::capture::{
    CapturedLocalEntryKind, SourceTreePolicy, capture_local_source,
    capture_local_source_from_open_root, open_absolute_directory_nofollow,
};
use super::model::{
    ResolvedLocalSnapshot, ResolvedLocalSource, VerifiedPackageSourceEntry,
    VerifiedPackageSourceEntryKind,
};
#[cfg(test)]
use super::snapshot::publish_local_snapshot;
use super::snapshot::publish_local_snapshot_in_lane;

pub fn resolve_local_source(
    root: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let limits = limits.compiler_bounded();
    Ok(capture_local_source(root.as_ref(), limits, SourceTreePolicy::LocalPackage)?.normalized)
}

/// Re-hash a published package snapshot under its original resolver limits.
///
/// This is a package-compilation custody check, not a defense against a
/// same-user process that can race both the verification and compiler reads.
pub(crate) fn verify_package_source_snapshot(
    root: &Path,
    expected: &SourceContentDigest,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    capture_verified_package_source_snapshot(root, expected, limits).map(|_| ())
}

/// Capture the exact bytes already covered by package-source custody.
///
/// Review-only consumers use this after transport resolution so they never
/// reopen a live checkout or infer a tree from package-authored ignore rules.
/// The returned paths are the same raw, root-relative bytes used by source
/// identity; every file and symlink payload has already participated in the
/// expected content commitment.
pub(crate) fn capture_verified_package_source_snapshot(
    root: &Path,
    expected: &SourceContentDigest,
    limits: LocalSourceLimits,
) -> Result<Vec<VerifiedPackageSourceEntry>, SourceResolveError> {
    let directory = open_absolute_directory_nofollow(root)
        .map_err(|error| local_snapshot_invalid(root, error.to_string()))?;
    verify_open_snapshot_tree_modes(CacheCustodyKind::LocalSnapshot, &directory, root)?;
    let captured = capture_local_source_from_open_root(
        root.to_path_buf(),
        directory,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?;
    let actual = SourceContentDigest::derive(captured.normalized.content_identity.as_bytes());
    if &actual != expected {
        return Err(SourceResolveError::SourceSnapshotContentMismatch {
            path: root.to_path_buf(),
            expected: expected.clone(),
            actual,
        });
    }
    Ok(captured
        .entries
        .into_iter()
        .map(|entry| VerifiedPackageSourceEntry {
            relative_path: entry.relative_bytes,
            kind: match entry.kind {
                CapturedLocalEntryKind::Directory => VerifiedPackageSourceEntryKind::Directory,
                CapturedLocalEntryKind::File { bytes, executable } => {
                    VerifiedPackageSourceEntryKind::File { bytes, executable }
                }
                CapturedLocalEntryKind::Symlink { target_bytes } => {
                    VerifiedPackageSourceEntryKind::Symlink { target_bytes }
                }
            },
        })
        .collect())
}

#[cfg(test)]
pub(crate) fn resolve_local_source_snapshot_at_path(
    root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    let limits = limits.compiler_bounded();
    let requested_root = root.as_ref().to_path_buf();
    let captured = capture_local_source(&requested_root, limits, SourceTreePolicy::LocalPackage)?;
    publish_local_snapshot(requested_root, captured, cache_dir.as_ref(), limits)
}

pub(crate) fn resolve_local_source_snapshot_in_lane(
    root: impl AsRef<Path>,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    let limits = limits.compiler_bounded();
    lane.verify_path_identity()?;
    let requested_root = root.as_ref().to_path_buf();
    let result = capture_local_source(&requested_root, limits, SourceTreePolicy::LocalPackage)
        .and_then(|captured| {
            publish_local_snapshot_in_lane(requested_root, captured, lane, limits)
        });
    match lane.verify_path_identity() {
        Ok(()) => result,
        Err(error) => Err(error),
    }
}

pub fn resolve_local_source_snapshot_with_storage(
    root: impl AsRef<Path>,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    storage.verify_path_identity()?;
    let result =
        resolve_local_source_snapshot_in_lane(root, storage.external_local_sources(), limits);
    storage.verify_path_identity()?;
    result
}
