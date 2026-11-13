/*
Bounded capture of a materialized package-source tree (`capture_local_source`,
`capture_exact_materialized`, and `capture_local_source_from_open_root`).

Because a retained root directory handle stays pinned to its original inode if
the root path is renamed or replaced mid-capture, `capture_local_source_from_open_root`
compares the canonical path's observation before and after traversal against the
retained handle's opening metadata, and runs `verify_captured_tree` after
selected-member capture so mutations landing in already-closed subtrees reject
before publication. Whole and scoped requests share the same selection, reads,
identity hashing, and no-follow closing checks.
*/

mod selection;
mod traversal;
mod traversal_observations;
pub(crate) use traversal::excluded_from_capture;

use std::path::{Path, PathBuf};

use cap_std::fs::Dir as CapabilityDirectory;

use traversal_observations::SourceEntryKind;
pub(crate) use traversal_observations::{
    CapturedLocalEntry, CapturedLocalEntryKind, CapturedLocalTree, SourceTreePolicy,
};

use super::ResolvedLocalSource;
use super::filesystem::{IoResultAt, open_canonical_source_root, require_unchanged_entry};
use super::identity::SourceIdentityHasher;
use crate::SourceResolveError;
use crate::error::too_many_files;
use crate::limits::{CANONICAL_DIRECTORY_MODE, LocalSourceLimits};
use filesystem_authority::SourceCaptureRequest;
use traversal::{CapturedTreeObservations, verify_captured_tree};

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
    capture_selected_local_source(
        requested_root,
        limits,
        policy,
        &SourceCaptureRequest::entire_tree(),
    )
}

pub(crate) fn capture_exact_materialized(
    root: &Path,
    root_directory: &CapabilityDirectory,
    limits: LocalSourceLimits,
) -> Result<CapturedLocalTree, SourceResolveError> {
    capture_local_source_from_open_root(
        root.to_path_buf(),
        root_directory.try_clone().at(root)?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )
}

pub(crate) fn capture_local_source_from_open_root(
    root: PathBuf,
    root_directory: CapabilityDirectory,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
) -> Result<CapturedLocalTree, SourceResolveError> {
    capture_selected_source_from_open_root(
        root,
        root_directory,
        limits,
        policy,
        &SourceCaptureRequest::entire_tree(),
    )
}

pub(crate) fn capture_selected_local_source(
    requested_root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
    selected: &SourceCaptureRequest,
) -> Result<CapturedLocalTree, SourceResolveError> {
    let root = requested_root.canonicalize().at(requested_root)?;
    if !root.is_dir() {
        return Err(SourceResolveError::NotDirectory { path: root });
    }
    let root_directory = open_canonical_source_root(&root)?;
    capture_selected_source_from_open_root(root, root_directory, limits, policy, selected)
}

fn capture_selected_source_from_open_root(
    root: PathBuf,
    root_directory: CapabilityDirectory,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
    selected: &SourceCaptureRequest,
) -> Result<CapturedLocalTree, SourceResolveError> {
    let mut source_entries = Vec::new();
    let opening_root = root_directory.dir_metadata().at(&root)?;
    let named_root = open_canonical_source_root(&root)?
        .dir_metadata()
        .at(&root)?;
    require_unchanged_entry(&opening_root, &named_root, &root)?;
    let mut tree_observations = CapturedTreeObservations::default();
    selection::capture_members(
        &root_directory,
        &root,
        limits,
        policy,
        selected,
        &mut source_entries,
        &mut tree_observations,
    )?;
    verify_captured_tree(&root_directory, &root, &tree_observations)?;
    let closing_root = open_canonical_source_root(&root)?
        .dir_metadata()
        .at(&root)?;
    require_unchanged_entry(&opening_root, &closing_root, &root)?;
    source_entries.sort_by(|left, right| left.relative_bytes.cmp(&right.relative_bytes));

    let mut identity = SourceIdentityHasher::new(source_entries.len());
    let mut file_count = 0;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(source_entries.len())
        .map_err(|_| too_many_files(limits.max_entries))?;
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
        selected: selected.clone(),
        normalized: ResolvedLocalSource {
            root,
            file_count,
            byte_count,
            content_identity,
        },
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_rejects_a_root_handle_for_a_different_directory() {
        let root = crate::test_support::temp_root("capture-root-identity");
        let first = root.join("first");
        let second = root.join("second");
        std::fs::create_dir_all(&first).expect("create first root");
        std::fs::create_dir(&second).expect("create second root");
        let retained = open_canonical_source_root(&first).expect("retain first root");
        let result = capture_local_source_from_open_root(
            second,
            retained,
            LocalSourceLimits::default(),
            SourceTreePolicy::ExactMaterialized,
        );
        assert!(matches!(
            result,
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
        std::fs::remove_dir_all(root).expect("remove fixture");
    }
}
