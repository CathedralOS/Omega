//! Source-patch model and rendering workflow.

use crate::declarations::PackageKey;
use crate::resolution::source::{PackageSourceCustody, PackageSourceSelectionEvidenceError};
use package_source::SourceResolveError;
use std::collections::BTreeSet;
use std::fmt;

use super::diff::DiffBudget;
#[cfg(test)]
use super::diff::{SourceLine, myers_diff, source_line_count, split_lines};
use super::output::BoundedOutput;
use super::snapshot::{
    capture_snapshot, file_content_requires_standalone_audit, render_entry, render_resolution,
    revalidate_snapshot, side_token,
};

const PATCH_SCHEMA: &str = "OMEGA_PACKAGE_SOURCE_PATCH_V1\n";
pub(super) const CONTEXT_LINES: usize = 3;

/// Independent ceilings for one model-facing source patch.
///
/// Snapshot bytes are capped before capture, diff work is counted across all
/// changed files, and output rejects rather than truncates. These are review
/// resource limits, not package identity or admission policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageSourcePatchLimits {
    maximum_entries_per_snapshot: usize,
    maximum_bytes_per_snapshot: u64,
    maximum_metadata_bytes_per_snapshot: usize,
    maximum_lines: usize,
    maximum_diff_work: usize,
    maximum_trace_cells: usize,
    maximum_output_bytes: usize,
}

impl PackageSourcePatchLimits {
    pub const fn new(
        maximum_entries_per_snapshot: usize,
        maximum_bytes_per_snapshot: u64,
        maximum_metadata_bytes_per_snapshot: usize,
        maximum_lines: usize,
        maximum_diff_work: usize,
        maximum_trace_cells: usize,
        maximum_output_bytes: usize,
    ) -> Self {
        Self {
            maximum_entries_per_snapshot,
            maximum_bytes_per_snapshot,
            maximum_metadata_bytes_per_snapshot,
            maximum_lines,
            maximum_diff_work,
            maximum_trace_cells,
            maximum_output_bytes,
        }
    }

    pub const fn maximum_entries_per_snapshot(self) -> usize {
        self.maximum_entries_per_snapshot
    }

    pub const fn maximum_bytes_per_snapshot(self) -> u64 {
        self.maximum_bytes_per_snapshot
    }

    pub const fn maximum_metadata_bytes_per_snapshot(self) -> usize {
        self.maximum_metadata_bytes_per_snapshot
    }

    pub const fn maximum_lines(self) -> usize {
        self.maximum_lines
    }

    pub const fn maximum_diff_work(self) -> usize {
        self.maximum_diff_work
    }

    pub const fn maximum_trace_cells(self) -> usize {
        self.maximum_trace_cells
    }

    pub const fn maximum_output_bytes(self) -> usize {
        self.maximum_output_bytes
    }
}

impl Default for PackageSourcePatchLimits {
    fn default() -> Self {
        Self::new(
            4_096,
            16 * 1024 * 1024,
            4 * 1024 * 1024,
            250_000,
            4_000_000,
            4_000_000,
            1024 * 1024,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSourcePatchSide {
    Baseline,
    Candidate,
}

#[derive(Debug)]
pub enum PackageSourcePatchError {
    PackageKeyMismatch,
    SourceCustody {
        side: PackageSourcePatchSide,
        error: SourceResolveError,
    },
    SourceSelectionCustody {
        side: PackageSourcePatchSide,
        error: PackageSourceSelectionEvidenceError,
    },
    SourceMetadataExceeded {
        side: PackageSourcePatchSide,
        maximum_bytes: usize,
    },
    TooManyLines {
        maximum: usize,
    },
    DiffWorkExceeded {
        maximum: usize,
    },
    DiffTraceExceeded {
        maximum_cells: usize,
    },
    OutputExceeded {
        maximum_bytes: usize,
        required_at_least: usize,
    },
}

impl fmt::Display for PackageSourcePatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PackageKeyMismatch => {
                write!(formatter, "cannot diff sources with different package keys")
            }
            Self::SourceCustody { side, error } => {
                write!(
                    formatter,
                    "{} source custody failed: {error}",
                    side_token(*side)
                )
            }
            Self::SourceSelectionCustody { side, error } => write!(
                formatter,
                "{} source selection custody failed: {error}",
                side_token(*side)
            ),
            Self::SourceMetadataExceeded {
                side,
                maximum_bytes,
            } => write!(
                formatter,
                "{} source path and symlink metadata exceeds the {maximum_bytes}-byte review ceiling",
                side_token(*side)
            ),
            Self::TooManyLines { maximum } => {
                write!(
                    formatter,
                    "source patch exceeds the {maximum}-line review ceiling"
                )
            }
            Self::DiffWorkExceeded { maximum } => {
                write!(
                    formatter,
                    "source patch exceeds the {maximum}-step diff-work ceiling"
                )
            }
            Self::DiffTraceExceeded { maximum_cells } => write!(
                formatter,
                "source patch exceeds the {maximum_cells}-cell diff-trace ceiling"
            ),
            Self::OutputExceeded {
                maximum_bytes,
                required_at_least,
            } => write!(
                formatter,
                "source patch requires at least {required_at_least} bytes, exceeding the {maximum_bytes}-byte review-output ceiling"
            ),
        }
    }
}

impl std::error::Error for PackageSourcePatchError {}

/// A bounded patch over compiler-consumed, resolver-owned source snapshots.
///
/// The fixed grammar and metadata are trusted renderer output. Source paths,
/// symlink payloads, and line bytes remain attacker-controlled code data; they
/// are lane-prefixed and byte-escaped but are not claimed prompt-safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourcePatch {
    baseline_key: Option<PackageKey>,
    candidate_key: PackageKey,
    changed_entries: usize,
    incomplete_model_content_entries: usize,
    rendered: String,
}

impl PackageSourcePatch {
    pub const fn baseline_key(&self) -> Option<&PackageKey> {
        self.baseline_key.as_ref()
    }

    pub const fn candidate_key(&self) -> &PackageKey {
        &self.candidate_key
    }

    pub const fn changed_entries(&self) -> usize {
        self.changed_entries
    }

    pub const fn is_empty(&self) -> bool {
        self.changed_entries == 0
    }

    pub const fn incomplete_model_content_entries(&self) -> usize {
        self.incomplete_model_content_entries
    }

    pub const fn requires_standalone_audit(&self) -> bool {
        self.incomplete_model_content_entries != 0
    }

    pub fn as_str(&self) -> &str {
        &self.rendered
    }

    pub fn into_string(self) -> String {
        self.rendered
    }
}

/// Render an update patch, or a complete candidate-source review when the old
/// snapshot is unavailable. Both snapshots are revalidated against their
/// resolver-issued content commitments before capture and after rendering.
pub fn render_package_source_patch(
    baseline: Option<&PackageSourceCustody>,
    candidate: &PackageSourceCustody,
    limits: PackageSourcePatchLimits,
) -> Result<PackageSourcePatch, PackageSourcePatchError> {
    if baseline.is_some_and(|baseline| baseline.key() != candidate.key()) {
        return Err(PackageSourcePatchError::PackageKeyMismatch);
    }

    let baseline_entries = baseline
        .map(|baseline| capture_snapshot(baseline, PackageSourcePatchSide::Baseline, limits))
        .transpose()?;
    let candidate_entries = capture_snapshot(candidate, PackageSourcePatchSide::Candidate, limits)?;
    let baseline_entries = baseline_entries.unwrap_or_default();
    let changed_paths = baseline_entries
        .keys()
        .chain(candidate_entries.keys())
        .filter(|path| baseline_entries.get(*path) != candidate_entries.get(*path))
        .cloned()
        .collect::<BTreeSet<_>>();

    let changed_entries = changed_paths.len();
    let incomplete_model_content_entries = changed_paths
        .iter()
        .filter(|path| {
            file_content_requires_standalone_audit(
                baseline_entries.get(*path),
                candidate_entries.get(*path),
            )
        })
        .count();
    let mut output = BoundedOutput::new(limits.maximum_output_bytes());
    output.push(PATCH_SCHEMA)?;
    output.push("mode ")?;
    output.push(if baseline.is_some() {
        "update\n"
    } else {
        "standalone_candidate\n"
    })?;
    output.push("package ")?;
    output.push(candidate.key().name().as_str())?;
    output.push("\n")?;
    output.push("baseline_key ")?;
    match baseline {
        Some(baseline) => output.push_hex(&baseline.key().identity().digest())?,
        None => output.push("none")?,
    }
    output.push("\n")?;
    output.push("candidate_key ")?;
    output.push_hex(&candidate.key().identity().digest())?;
    output.push("\n")?;
    render_resolution(
        &mut output,
        "baseline",
        baseline.map(PackageSourceCustody::resolution),
    )?;
    render_resolution(&mut output, "candidate", Some(candidate.resolution()))?;
    output.push("changed_entries ")?;
    output.push_usize(changed_entries)?;
    output.push("\nincomplete_model_content_entries ")?;
    output.push_usize(incomplete_model_content_entries)?;
    output.push("\n")?;

    let mut budget = DiffBudget::new(limits);
    for path in &changed_paths {
        render_entry(
            &mut output,
            &mut budget,
            path,
            baseline_entries.get(path),
            candidate_entries.get(path),
        )?;
    }
    output.push("end_source_patch\n")?;

    if let Some(baseline) = baseline {
        revalidate_snapshot(baseline, PackageSourcePatchSide::Baseline)?;
    }
    revalidate_snapshot(candidate, PackageSourcePatchSide::Candidate)?;

    Ok(PackageSourcePatch {
        baseline_key: baseline.map(|baseline| baseline.key().clone()),
        candidate_key: candidate.key().clone(),
        changed_entries,
        incomplete_model_content_entries,
        rendered: output.finish(),
    })
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
