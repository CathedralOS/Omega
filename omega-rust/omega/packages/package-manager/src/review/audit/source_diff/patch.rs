//! Source-patch model and rendering workflow.

use crate::declarations::PackageKey;
use crate::resolution::source::{PackageSourceCustody, PackageSourceSelectionEvidenceError};
use crate::review::timings;
use package_source::SourceResolveError;
use std::collections::BTreeSet;
use std::fmt;

use super::MAXIMUM_OUTPUT_BYTES;
use super::diff::DiffBudget;
use super::output::BoundedOutput;
use super::snapshot::{
    capture_snapshot, file_content_requires_standalone_audit, render_entry, render_resolution,
    revalidate_snapshot, side_token,
};

const PATCH_SCHEMA: &str = "OMEGA_PACKAGE_SOURCE_PATCH_V1\n";
pub(super) const CONTEXT_LINES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackageSourcePatchSide {
    Baseline,
    Candidate,
}

#[derive(Debug)]
pub(crate) enum PackageSourcePatchError {
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
pub(crate) struct PackageSourcePatch {
    baseline_key: Option<PackageKey>,
    candidate_key: PackageKey,
    changed_entries: usize,
    incomplete_model_content_entries: usize,
    rendered: String,
}

impl PackageSourcePatch {
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
/// snapshot is unavailable. Both snapshots are verified against their
/// resolver-issued content commitments inside capture and revalidated after
/// rendering.
pub(crate) fn render_package_source_patch(
    baseline: Option<&PackageSourceCustody>,
    candidate: &PackageSourceCustody,
) -> Result<PackageSourcePatch, PackageSourcePatchError> {
    let _stage = timings::stage("source_patch_rendering");
    if baseline.is_some_and(|baseline| baseline.key() != candidate.key()) {
        return Err(PackageSourcePatchError::PackageKeyMismatch);
    }

    let baseline_entries = baseline
        .map(|baseline| capture_snapshot(baseline, PackageSourcePatchSide::Baseline))
        .transpose()?;
    let candidate_entries = capture_snapshot(candidate, PackageSourcePatchSide::Candidate)?;
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
    let mut output = BoundedOutput::new(MAXIMUM_OUTPUT_BYTES);
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

    let mut budget = DiffBudget::new();
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
