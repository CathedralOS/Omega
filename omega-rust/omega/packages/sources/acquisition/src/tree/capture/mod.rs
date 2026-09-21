//! Bounded capture of a materialized package-source tree.

mod traversal;
mod traversal_observations;

use std::path::{Path, PathBuf};

use cap_std::fs::Dir as CapabilityDirectory;

use traversal_observations::SourceEntryKind;
pub(crate) use traversal_observations::{
    CapturedLocalEntry, CapturedLocalEntryKind, CapturedLocalTree, SourceTreePolicy,
};

use super::ResolvedLocalSource;
use super::filesystem::{io_error, open_canonical_source_root, require_unchanged_entry};
use super::identity::SourceIdentityHasher;
use crate::SourceResolveError;
use crate::limits::{CANONICAL_DIRECTORY_MODE, LocalSourceLimits};
use traversal::{CapturedTreeObservations, verify_captured_tree, visit_directory};

#[cfg(test)]
pub(crate) fn resolve_materialized_source(
    root: &Path,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    Ok(capture_local_source(root, limits, SourceTreePolicy::ExactMaterialized)?.normalized)
}

pub(crate) fn capture_local_source(
    requested_root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
) -> Result<CapturedLocalTree, SourceResolveError> {
    let root = requested_root
        .canonicalize()
        .map_err(|error| io_error(requested_root, error))?;
    if !root.is_dir() {
        return Err(SourceResolveError::NotDirectory { path: root });
    }

    let root_directory = open_canonical_source_root(&root)?;
    capture_local_source_from_open_root(root, root_directory, limits, policy)
}

pub(crate) fn capture_local_source_from_open_root(
    root: PathBuf,
    root_directory: CapabilityDirectory,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
) -> Result<CapturedLocalTree, SourceResolveError> {
    let mut source_entries = Vec::new();
    let mut captured_file_bytes = 0_u64;
    // The retained root handle cannot see its own replacement: a renamed or
    // remade root leaves the handle pinned to the old inode while the path
    // moves on. Compare the canonical path's observation before and after the
    // traversal so that swap rejects instead of publishing a stale capture.
    let opening_root = std::fs::symlink_metadata(&root).map_err(|error| io_error(&root, error))?;
    let mut tree_observations = CapturedTreeObservations::default();
    visit_directory(
        &root_directory,
        &root_directory,
        &root,
        PathBuf::new(),
        0,
        &root,
        limits,
        policy,
        &mut captured_file_bytes,
        &mut source_entries,
        &mut tree_observations,
    )?;
    // The per-directory close covers drift while its own visit ran; this
    // sweep re-observes the whole recorded tree once more so a mutation that
    // landed in an already-closed subtree still rejects at capture time.
    verify_captured_tree(&root_directory, &root, &tree_observations)?;
    let closing_root = std::fs::symlink_metadata(&root).map_err(|error| io_error(&root, error))?;
    require_unchanged_entry(
        &cap_std::fs::Metadata::from_just_metadata(opening_root),
        &cap_std::fs::Metadata::from_just_metadata(closing_root),
        &root,
    )?;
    source_entries.sort_by(|left, right| left.relative_bytes.cmp(&right.relative_bytes));

    let mut identity = SourceIdentityHasher::new(source_entries.len());
    let mut file_count = 0;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(source_entries.len())
        .map_err(|_| SourceResolveError::TooManyFiles {
            limit: limits.max_entries,
        })?;
    for entry in source_entries {
        let kind = match entry.kind {
            SourceEntryKind::Directory => {
                identity.add_directory(&entry.relative_bytes, CANONICAL_DIRECTORY_MODE);
                CapturedLocalEntryKind::Directory
            }
            SourceEntryKind::File { bytes, executable } => {
                identity.add_file(&entry.relative_bytes, executable, &bytes)?;
                file_count += 1;
                CapturedLocalEntryKind::File { bytes, executable }
            }
            SourceEntryKind::Symlink { target_bytes } => {
                identity.add_symlink(&entry.relative_bytes, &target_bytes);
                file_count += 1;
                CapturedLocalEntryKind::Symlink { target_bytes }
            }
        };
        entries.push(CapturedLocalEntry {
            relative_path: entry.relative_path,
            relative_bytes: entry.relative_bytes,
            kind,
        });
    }
    let (byte_count, content_identity) = identity.finish();
    Ok(CapturedLocalTree {
        normalized: ResolvedLocalSource {
            root,
            file_count,
            byte_count,
            content_identity,
        },
        entries,
    })
}
