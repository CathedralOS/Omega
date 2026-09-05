//! Proposed local source edits, without changing the live project.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::model::ResolvedLocalSource;
use super::snapshot::{
    publish_local_tree_in_lane, verify_live_source_unchanged, verify_requested_local_root,
};
use crate::SourceResolveError;
use crate::identity::SourceRelativePath;
use crate::limits::{CANONICAL_DIRECTORY_MODE, LocalSourceLimits};
use crate::storage::RetainedStorageLane;
use crate::tree::capture::{
    CapturedLocalEntryKind, CapturedLocalTree, SourceTreePolicy, capture_local_source,
};
use crate::tree::identity::SourceIdentityHasher;

/// A proposed immutable tree and the live source version it would replace.
///
/// Its package lineage remains the original local path. It is not an
/// observation that the proposed bytes already exist in the live project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedLocalSnapshot {
    requested_root: PathBuf,
    original: ResolvedLocalSource,
    normalized: ResolvedLocalSource,
    limits: LocalSourceLimits,
}

impl StagedLocalSnapshot {
    pub fn requested_root(&self) -> &Path {
        &self.requested_root
    }

    pub fn canonical_live_root(&self) -> &Path {
        &self.original.root
    }

    pub const fn original(&self) -> &ResolvedLocalSource {
        &self.original
    }

    pub const fn normalized(&self) -> &ResolvedLocalSource {
        &self.normalized
    }

    pub fn snapshot_root(&self) -> &Path {
        &self.normalized.root
    }

    /// Detect edits since staging before the caller publishes project files.
    /// Publication still needs its own immediate file checks and recovery.
    pub fn verify_live_source_unchanged(&self) -> Result<(), SourceResolveError> {
        verify_requested_local_root(&self.requested_root, &self.original.root)?;
        verify_live_source_unchanged(&self.original, self.limits)
    }
}

/// Replace one existing regular source file in an immutable candidate snapshot.
/// The expected SHA-256 is over the old file bytes, not its source-tree digest.
pub fn stage_local_source_replacement_in_lane(
    root: &Path,
    relative_path: &SourceRelativePath,
    expected_sha256: &[u8; 32],
    replacement: &[u8],
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<StagedLocalSnapshot, SourceResolveError> {
    let limits = limits.compiler_bounded();
    lane.verify_path_identity()?;
    let mut captured = capture_local_source(root, limits, SourceTreePolicy::LocalPackage)?;
    let original = captured.normalized.clone();
    replace_file(
        &mut captured,
        relative_path,
        expected_sha256,
        replacement,
        limits,
    )?;
    let normalized = publish_local_tree_in_lane(root, &captured, &original, lane, limits)?;
    Ok(StagedLocalSnapshot {
        requested_root: root.to_path_buf(),
        original,
        normalized,
        limits,
    })
}

fn replace_file(
    captured: &mut CapturedLocalTree,
    relative_path: &SourceRelativePath,
    expected_sha256: &[u8; 32],
    replacement: &[u8],
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    let path = captured.normalized.root.join(relative_path.as_str());
    let entry = captured
        .entries
        .iter_mut()
        .find(|entry| entry.relative_bytes == relative_path.as_str().as_bytes());
    let Some(crate::tree::capture::CapturedLocalEntry {
        kind: CapturedLocalEntryKind::File { bytes, .. },
        ..
    }) = entry
    else {
        return Err(SourceResolveError::LocalSourceReplacementInvalid {
            path,
            message: "replacement requires an existing regular captured source file".to_owned(),
        });
    };
    let actual: [u8; 32] = Sha256::digest(bytes.as_slice()).into();
    if &actual != expected_sha256 {
        return Err(SourceResolveError::LocalSourceChanged { path });
    }
    let proposed_bytes = captured
        .normalized
        .byte_count
        .checked_sub(bytes.len() as u64)
        .and_then(|remaining| remaining.checked_add(replacement.len() as u64))
        .filter(|total| *total <= limits.max_bytes)
        .ok_or(SourceResolveError::TooManyBytes {
            limit: limits.max_bytes,
        })?;
    let mut proposed = Vec::new();
    proposed.try_reserve_exact(replacement.len()).map_err(|_| {
        SourceResolveError::LocalSourceReplacementInvalid {
            path,
            message: "replacement bytes could not be allocated".to_owned(),
        }
    })?;
    proposed.extend_from_slice(replacement);
    *bytes = proposed;

    // Entry order and executable bits are unchanged. Use the same exact-tree
    // encoding as normal capture, so landing these bytes yields the same pin.
    let mut identity = SourceIdentityHasher::new(captured.entries.len());
    for entry in &captured.entries {
        match &entry.kind {
            CapturedLocalEntryKind::Directory => {
                identity.add_directory(&entry.relative_bytes, CANONICAL_DIRECTORY_MODE);
            }
            CapturedLocalEntryKind::File { bytes, executable } => {
                identity.add_file(&entry.relative_bytes, *executable, bytes)?;
            }
            CapturedLocalEntryKind::Symlink { target_bytes } => {
                identity.add_symlink(&entry.relative_bytes, target_bytes);
            }
        }
    }
    let (byte_count, content_identity) = identity.finish();
    debug_assert_eq!(byte_count, proposed_bytes);
    captured.normalized.byte_count = byte_count;
    captured.normalized.content_identity = content_identity;
    Ok(())
}
