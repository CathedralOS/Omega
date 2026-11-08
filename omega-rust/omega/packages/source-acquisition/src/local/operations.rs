//! Public local resolution and package-snapshot verification operations.

pub use super::recovery::recover_cached_local_source_in_lane;

use std::path::Path;
use std::sync::Arc;

use crate::SourceResolveError;
#[cfg(test)]
use crate::SourceResolver;
use crate::custody::tree::CacheCustodyKind;
use crate::error::local_snapshot_invalid;
use crate::identity::SourceContentDigest;
use crate::limits::LocalSourceLimits;
use crate::snapshot::permissions::verify_open_snapshot_tree_modes;
use crate::storage::RetainedStorageLane;

use super::resolution_observations::{
    CapturedLocalSource, ResolvedLocalSnapshot, ResolvedLocalSource, VerifiedPackageSourceEntry,
    VerifiedPackageSourceEntryKind,
};
use super::snapshot::publish_local_snapshot_in_lane;
use crate::tree::capture::{
    CapturedLocalEntryKind, SourceTreePolicy, capture_local_source,
    capture_local_source_from_open_root,
};
use crate::tree::filesystem::open_absolute_directory_nofollow;

pub fn resolve_local_source(
    root: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let limits = limits.compiler_bounded();
    Ok(capture_local_source(root.as_ref(), limits, SourceTreePolicy::LocalPackage)?.normalized)
}

/// Acquire once, retaining the same bytes that produced source identity.
/// No disk publication or later checkout comparison is needed: every consumer
/// uses this closed inventory. Capture still rejects observed tree drift.
pub fn capture_local_source_input(
    root: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<CapturedLocalSource, SourceResolveError> {
    let requested_root = root.as_ref().to_path_buf();
    let captured = capture_local_source(
        &requested_root,
        limits.compiler_bounded(),
        SourceTreePolicy::LocalPackage,
    )?;
    let mut entries = Vec::with_capacity(captured.entries.len());
    for entry in captured.entries {
        let node = match entry.kind {
            CapturedLocalEntryKind::Directory => source::CapturedSourceNode::Directory,
            CapturedLocalEntryKind::File { bytes, executable } => {
                source::CapturedSourceNode::File {
                    bytes: Arc::from(bytes),
                    executable,
                }
            }
            CapturedLocalEntryKind::Symlink { target_bytes } => {
                source::CapturedSourceNode::Symlink {
                    target: captured_link_path(target_bytes, &requested_root)?,
                }
            }
        };
        entries.push((entry.relative_path, node));
    }
    let tree = source::CapturedSourceTree::new(entries)
        .map_err(|error| crate::tree::filesystem::io_error(&requested_root, error))?;
    Ok(CapturedLocalSource {
        requested_root,
        normalized: captured.normalized,
        tree: Arc::new(tree),
    })
}

#[cfg(unix)]
fn captured_link_path(
    bytes: Vec<u8>,
    _root: &Path,
) -> Result<std::path::PathBuf, SourceResolveError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(std::ffi::OsString::from_vec(bytes).into())
}

#[cfg(not(unix))]
fn captured_link_path(
    bytes: Vec<u8>,
    root: &Path,
) -> Result<std::path::PathBuf, SourceResolveError> {
    String::from_utf8(bytes)
        .map(std::path::PathBuf::from)
        .map_err(|error| SourceResolveError::Io {
            path: root.to_path_buf(),
            message: error.to_string(),
        })
}

/// Re-hash a published package snapshot under its original resolver limits.
///
/// This detects source drift at the verification boundary. Processes already
/// holding the invoking user's host authority remain outside its claim.
pub fn verify_package_source_snapshot(
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
pub fn capture_verified_package_source_snapshot(
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

pub fn resolve_local_source_snapshot_in_lane(
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

/// Resolve and publish a local source through a caller-supplied resolver.
///
/// Test fixture entry: production callers publish into an explicit lane via
/// [`resolve_local_source_snapshot_in_lane`].
#[cfg(test)]
pub(crate) fn resolve_local_source_snapshot(
    root: impl AsRef<Path>,
    resolver: &SourceResolver,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    resolver.verify_path_identity()?;
    let result =
        resolve_local_source_snapshot_in_lane(root, resolver.external_local_sources(), limits);
    resolver.verify_path_identity()?;
    result
}
