//! Snapshot capture, entry classification, and fixed metadata rendering.

use super::diff::{DiffBudget, myers_diff, render_hunks, source_line_count, split_lines};
use super::output::BoundedOutput;
use super::{PackageSourcePatchError, PackageSourcePatchLimits, PackageSourcePatchSide};
use crate::{GitObjectIdAlgorithm, ImmutableSourceResolution, PackageSourceCustody};
use omega_package_source::{
    LocalSourceLimits, VerifiedPackageSourceEntry, VerifiedPackageSourceEntryKind,
    capture_verified_package_source_snapshot, verify_package_source_snapshot,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(super) fn render_resolution(
    output: &mut BoundedOutput,
    side: &str,
    resolution: Option<&ImmutableSourceResolution>,
) -> Result<(), PackageSourcePatchError> {
    output.push(side)?;
    output.push("_resolution_kind ")?;
    match resolution {
        None => output.push("absent\n")?,
        Some(ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        }) => {
            output.push("git\n")?;
            output.push(side)?;
            output.push("_git_object_format ")?;
            output.push(match commit.algorithm() {
                GitObjectIdAlgorithm::Sha1 => "sha1\n",
                GitObjectIdAlgorithm::Sha256 => "sha256\n",
            })?;
            output.push(side)?;
            output.push("_git_commit ")?;
            output.push(&commit.to_hex())?;
            output.push("\n")?;
            output.push(side)?;
            output.push("_git_tree ")?;
            output.push(&tree.to_hex())?;
            output.push("\n")?;
            output.push(side)?;
            output.push("_source_content ")?;
            output.push(&content.to_hex())?;
            output.push("\n")?;
        }
        Some(ImmutableSourceResolution::Workspace { content }) => {
            output.push("workspace\n")?;
            output.push(side)?;
            output.push("_source_content ")?;
            output.push(&content.to_hex())?;
            output.push("\n")?;
        }
        Some(ImmutableSourceResolution::ExternalLocal { content }) => {
            output.push("external_local\n")?;
            output.push(side)?;
            output.push("_source_content ")?;
            output.push(&content.to_hex())?;
            output.push("\n")?;
        }
    }
    Ok(())
}

pub(super) fn capture_snapshot(
    custody: &PackageSourceCustody,
    side: PackageSourcePatchSide,
    limits: PackageSourcePatchLimits,
) -> Result<BTreeMap<Vec<u8>, VerifiedPackageSourceEntryKind>, PackageSourcePatchError> {
    let custody_limits = custody.source_limits();
    let review_limits = LocalSourceLimits {
        max_files: custody_limits
            .max_files
            .min(limits.maximum_entries_per_snapshot()),
        max_bytes: custody_limits
            .max_bytes
            .min(limits.maximum_bytes_per_snapshot()),
        max_depth: custody_limits.max_depth,
    };
    let entries = capture_verified_package_source_snapshot(
        custody.snapshot_root(),
        custody.resolution().content(),
        review_limits,
    )
    .map_err(|error| PackageSourcePatchError::SourceCustody { side, error })?;
    let mut metadata_bytes = 0_usize;
    let mut captured = BTreeMap::new();
    for VerifiedPackageSourceEntry {
        relative_path,
        kind,
    } in entries
    {
        metadata_bytes = metadata_bytes
            .checked_add(relative_path.len())
            .and_then(|total| match &kind {
                VerifiedPackageSourceEntryKind::Symlink { target_bytes } => {
                    total.checked_add(target_bytes.len())
                }
                _ => Some(total),
            })
            .ok_or(PackageSourcePatchError::SourceMetadataExceeded {
                side,
                maximum_bytes: limits.maximum_metadata_bytes_per_snapshot(),
            })?;
        if metadata_bytes > limits.maximum_metadata_bytes_per_snapshot() {
            return Err(PackageSourcePatchError::SourceMetadataExceeded {
                side,
                maximum_bytes: limits.maximum_metadata_bytes_per_snapshot(),
            });
        }
        captured.insert(relative_path, kind);
    }
    Ok(captured)
}

pub(super) fn revalidate_snapshot(
    custody: &PackageSourceCustody,
    side: PackageSourcePatchSide,
) -> Result<(), PackageSourcePatchError> {
    verify_package_source_snapshot(
        custody.snapshot_root(),
        custody.resolution().content(),
        custody.source_limits(),
    )
    .map_err(|error| PackageSourcePatchError::SourceCustody { side, error })
}

pub(super) fn render_entry(
    output: &mut BoundedOutput,
    budget: &mut DiffBudget,
    path: &[u8],
    baseline: Option<&VerifiedPackageSourceEntryKind>,
    candidate: Option<&VerifiedPackageSourceEntryKind>,
) -> Result<(), PackageSourcePatchError> {
    output.push("entry ")?;
    output.push_escaped(path)?;
    output.push("\nbaseline_kind ")?;
    output.push(entry_kind_token(baseline))?;
    output.push("\ncandidate_kind ")?;
    output.push(entry_kind_token(candidate))?;
    output.push("\n")?;

    if let Some(VerifiedPackageSourceEntryKind::Symlink { target_bytes }) = baseline {
        output.push("baseline_target ")?;
        output.push_escaped(target_bytes)?;
        output.push("\n")?;
    }
    if let Some(VerifiedPackageSourceEntryKind::Symlink { target_bytes }) = candidate {
        output.push("candidate_target ")?;
        output.push_escaped(target_bytes)?;
        output.push("\n")?;
    }

    let baseline_file = file_parts(baseline);
    let candidate_file = file_parts(candidate);
    if baseline_file.is_some() || candidate_file.is_some() {
        output.push("baseline_executable ")?;
        output.push(optional_bool_token(
            baseline_file.map(|(_, executable)| executable),
        ))?;
        output.push("\ncandidate_executable ")?;
        output.push(optional_bool_token(
            candidate_file.map(|(_, executable)| executable),
        ))?;
        output.push("\n")?;
        let baseline_bytes = baseline_file.map_or(&[][..], |(bytes, _)| bytes);
        let candidate_bytes = candidate_file.map_or(&[][..], |(bytes, _)| bytes);
        if file_payloads_equal(baseline_file, candidate_file) {
            output.push("content_review unchanged\n")?;
        } else if !is_model_text(baseline_bytes) || !is_model_text(candidate_bytes) {
            output.push("content_review unavailable_binary_or_non_utf8\n")?;
            render_file_summary(output, "baseline", baseline_file.map(|(bytes, _)| bytes))?;
            render_file_summary(output, "candidate", candidate_file.map(|(bytes, _)| bytes))?;
        } else {
            output.push("content_review complete_text_patch\n")?;
            let baseline_line_count = source_line_count(baseline_bytes);
            let candidate_line_count = source_line_count(candidate_bytes);
            budget.add_lines(baseline_line_count, candidate_line_count)?;
            let baseline_lines =
                split_lines(baseline_bytes, baseline_line_count, budget.maximum_lines)?;
            let candidate_lines =
                split_lines(candidate_bytes, candidate_line_count, budget.maximum_lines)?;
            let edits = myers_diff(&baseline_lines, &candidate_lines, budget)?;
            render_hunks(output, &baseline_lines, &candidate_lines, &edits)?;
        }
    }
    output.push("end_entry\n")
}

fn file_payloads_equal(baseline: Option<(&[u8], bool)>, candidate: Option<(&[u8], bool)>) -> bool {
    matches!((baseline, candidate), (Some((old, _)), Some((new, _))) if old == new)
}

pub(super) fn file_content_requires_standalone_audit(
    baseline: Option<&VerifiedPackageSourceEntryKind>,
    candidate: Option<&VerifiedPackageSourceEntryKind>,
) -> bool {
    let baseline = file_parts(baseline).map(|(bytes, _)| bytes);
    let candidate = file_parts(candidate).map(|(bytes, _)| bytes);
    let baseline_bytes = baseline.unwrap_or_default();
    let candidate_bytes = candidate.unwrap_or_default();
    baseline_bytes != candidate_bytes
        && (!is_model_text(baseline_bytes) || !is_model_text(candidate_bytes))
}

fn is_model_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

fn render_file_summary(
    output: &mut BoundedOutput,
    side: &str,
    bytes: Option<&[u8]>,
) -> Result<(), PackageSourcePatchError> {
    output.push(side)?;
    output.push("_bytes ")?;
    match bytes {
        Some(bytes) => output.push_usize(bytes.len())?,
        None => output.push("absent")?,
    }
    output.push("\n")?;
    output.push(side)?;
    output.push("_content_commitment ")?;
    match bytes {
        Some(bytes) => output.push_hex(&file_content_commitment(bytes))?,
        None => output.push("absent")?,
    }
    output.push("\n")
}

fn file_content_commitment(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"omega-package-source-file-v1\0");
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn file_parts(kind: Option<&VerifiedPackageSourceEntryKind>) -> Option<(&[u8], bool)> {
    match kind {
        Some(VerifiedPackageSourceEntryKind::File { bytes, executable }) => {
            Some((bytes, *executable))
        }
        _ => None,
    }
}

const fn entry_kind_token(kind: Option<&VerifiedPackageSourceEntryKind>) -> &'static str {
    match kind {
        None => "absent",
        Some(VerifiedPackageSourceEntryKind::Directory) => "directory",
        Some(VerifiedPackageSourceEntryKind::File { .. }) => "file",
        Some(VerifiedPackageSourceEntryKind::Symlink { .. }) => "symlink",
    }
}

const fn optional_bool_token(value: Option<bool>) -> &'static str {
    match value {
        None => "absent",
        Some(false) => "false",
        Some(true) => "true",
    }
}

pub(super) const fn side_token(side: PackageSourcePatchSide) -> &'static str {
    match side {
        PackageSourcePatchSide::Baseline => "baseline",
        PackageSourcePatchSide::Candidate => "candidate",
    }
}
