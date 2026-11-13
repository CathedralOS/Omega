// Public local resolution, in-memory capture, and package-snapshot verification
// operations.
//
// `capture_local_source_input` acquires a local package once into a closed
// in-memory `CapturedLocalSource` inventory so downstream consumers never
// reopen the live checkout. `capture_verified_package_source_snapshot` and
// `verify_package_source_snapshot` re-verify an already-published snapshot tree
// under `SourceTreePolicy::ExactMaterialized` against its expected
// `SourceContentDigest`, enabling review-only consumers to inspect exact
// custody-backed bytes without re-evaluating ignore rules.
// `resolve_local_source_snapshot_in_lane` captures and publishes a local
// snapshot inside a retained storage lane, checking lane path identity before
// and after publication.

pub use super::recovery::recover_cached_local_source_in_lane;

use filesystem_authority::{SourceCaptureObligation, SourceCaptureRequest};
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
    capture_local_source_from_open_root, capture_selected_local_source,
};
use crate::tree::filesystem::open_absolute_directory_nofollow;

pub fn resolve_local_source(
    root: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let limits = limits.compiler_bounded();
    Ok(capture_local_source(root.as_ref(), limits, SourceTreePolicy::LocalPackage)?.normalized)
}

pub fn capture_local_source_input(
    root: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<CapturedLocalSource, SourceResolveError> {
    capture_selected_local_source_input(root, limits, &SourceCaptureRequest::entire_tree())
}

/// Capture caller-authorized source membership before any source consumer can
/// discover declarations or request Build reads from the immutable namespace.
pub fn capture_selected_local_source_input(
    root: impl AsRef<Path>,
    limits: LocalSourceLimits,
    selected: &SourceCaptureRequest,
) -> Result<CapturedLocalSource, SourceResolveError> {
    let requested_root = root.as_ref().to_path_buf();
    let captured = capture_selected_local_source(
        &requested_root,
        limits.compiler_bounded(),
        SourceTreePolicy::LocalPackage,
        selected,
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
        selection_identity: selection_identity(&captured.selected),
    })
}

pub(crate) fn selection_identity(selected: &SourceCaptureRequest) -> [u8; 32] {
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega-source-selection-v1\0");
    digest.u64(selected.entries().count() as u64);
    for (path, obligation) in selected.entries() {
        digest.bytes(path);
        digest.byte(match obligation {
            SourceCaptureObligation::Required => 0,
            SourceCaptureObligation::Optional => 1,
        });
    }
    digest.finish()
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

pub fn verify_package_source_snapshot(
    root: &Path,
    expected: &SourceContentDigest,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    capture_verified_package_source_snapshot(root, expected, limits).map(|_| ())
}

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
