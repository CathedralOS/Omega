use crate::source::{
    LocalSourceLimits, SourceResolveError, VerifiedPackageSourceEntry,
    VerifiedPackageSourceEntryKind, capture_verified_package_source_snapshot,
    verify_package_source_snapshot,
};
use crate::{GitObjectIdAlgorithm, ImmutableSourceResolution, PackageSourceCustody};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const PATCH_SCHEMA: &str = "OMEGA_PACKAGE_SOURCE_PATCH_V1\n";
const CONTEXT_LINES: usize = 3;

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
    changed_entries: usize,
    incomplete_model_content_entries: usize,
    rendered: String,
}

impl PackageSourcePatch {
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
        changed_entries,
        incomplete_model_content_entries,
        rendered: output.finish(),
    })
}

fn render_resolution(
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

fn capture_snapshot(
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

fn revalidate_snapshot(
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

fn render_entry(
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

fn file_content_requires_standalone_audit(
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

const fn side_token(side: PackageSourcePatchSide) -> &'static str {
    match side {
        PackageSourcePatchSide::Baseline => "baseline",
        PackageSourcePatchSide::Candidate => "candidate",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceLine<'source> {
    bytes: &'source [u8],
    has_lf: bool,
}

fn source_line_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count()
        + usize::from(!bytes.is_empty() && !bytes.ends_with(b"\n"))
}

fn split_lines(
    bytes: &[u8],
    line_count: usize,
    maximum_lines: usize,
) -> Result<Vec<SourceLine<'_>>, PackageSourcePatchError> {
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(line_count)
        .map_err(|_| PackageSourcePatchError::TooManyLines {
            maximum: maximum_lines,
        })?;
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            lines.push(SourceLine {
                bytes: &bytes[start..index],
                has_lf: true,
            });
            start = index + 1;
        }
    }
    if start < bytes.len() {
        lines.push(SourceLine {
            bytes: &bytes[start..],
            has_lf: false,
        });
    }
    debug_assert_eq!(lines.len(), line_count);
    Ok(lines)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edit {
    Equal { baseline: usize, candidate: usize },
    Remove { baseline: usize },
    Add { candidate: usize },
}

impl Edit {
    const fn is_change(self) -> bool {
        !matches!(self, Self::Equal { .. })
    }
}

struct DiffBudget {
    maximum_lines: usize,
    maximum_work: usize,
    maximum_trace_cells: usize,
    lines: usize,
    work: usize,
    trace_cells: usize,
}

impl DiffBudget {
    const fn new(limits: PackageSourcePatchLimits) -> Self {
        Self {
            maximum_lines: limits.maximum_lines(),
            maximum_work: limits.maximum_diff_work(),
            maximum_trace_cells: limits.maximum_trace_cells(),
            lines: 0,
            work: 0,
            trace_cells: 0,
        }
    }

    fn add_lines(
        &mut self,
        baseline: usize,
        candidate: usize,
    ) -> Result<(), PackageSourcePatchError> {
        self.lines = self
            .lines
            .checked_add(baseline)
            .and_then(|lines| lines.checked_add(candidate))
            .ok_or(PackageSourcePatchError::TooManyLines {
                maximum: self.maximum_lines,
            })?;
        if self.lines > self.maximum_lines {
            return Err(PackageSourcePatchError::TooManyLines {
                maximum: self.maximum_lines,
            });
        }
        Ok(())
    }

    fn work(&mut self) -> Result<(), PackageSourcePatchError> {
        self.work = self
            .work
            .checked_add(1)
            .ok_or(PackageSourcePatchError::DiffWorkExceeded {
                maximum: self.maximum_work,
            })?;
        if self.work > self.maximum_work {
            return Err(PackageSourcePatchError::DiffWorkExceeded {
                maximum: self.maximum_work,
            });
        }
        Ok(())
    }

    fn trace(&mut self, cells: usize) -> Result<(), PackageSourcePatchError> {
        self.trace_cells = self.trace_cells.checked_add(cells).ok_or(
            PackageSourcePatchError::DiffTraceExceeded {
                maximum_cells: self.maximum_trace_cells,
            },
        )?;
        if self.trace_cells > self.maximum_trace_cells {
            return Err(PackageSourcePatchError::DiffTraceExceeded {
                maximum_cells: self.maximum_trace_cells,
            });
        }
        Ok(())
    }
}

fn myers_diff(
    baseline: &[SourceLine<'_>],
    candidate: &[SourceLine<'_>],
    budget: &mut DiffBudget,
) -> Result<Vec<Edit>, PackageSourcePatchError> {
    let maximum = baseline.len().checked_add(candidate.len()).ok_or(
        PackageSourcePatchError::TooManyLines {
            maximum: budget.maximum_lines,
        },
    )?;
    if maximum == 0 {
        return Ok(Vec::new());
    }
    let width = maximum
        .checked_mul(2)
        .and_then(|width| width.checked_add(1))
        .ok_or(PackageSourcePatchError::DiffTraceExceeded {
            maximum_cells: budget.maximum_trace_cells,
        })?;
    let offset = isize::try_from(maximum).map_err(|_| PackageSourcePatchError::TooManyLines {
        maximum: budget.maximum_lines,
    })?;
    budget.trace(width)?;
    let mut frontier = Vec::new();
    frontier
        .try_reserve_exact(width)
        .map_err(|_| PackageSourcePatchError::DiffTraceExceeded {
            maximum_cells: budget.maximum_trace_cells,
        })?;
    frontier.resize(width, -1_isize);
    frontier[(offset + 1) as usize] = 0;
    let mut trace = Vec::new();

    for distance in 0..=maximum {
        let distance = isize::try_from(distance).expect("diff distance fits isize");
        let mut diagonal = -distance;
        while diagonal <= distance {
            budget.work()?;
            let index = usize::try_from(offset + diagonal).expect("frontier index is nonnegative");
            let mut x = if diagonal == -distance
                || (diagonal != distance && frontier[index - 1] < frontier[index + 1])
            {
                frontier[index + 1]
            } else {
                frontier[index - 1] + 1
            };
            let mut y = x - diagonal;
            while x < baseline.len() as isize
                && y < candidate.len() as isize
                && baseline[x as usize] == candidate[y as usize]
            {
                budget.work()?;
                x += 1;
                y += 1;
            }
            frontier[index] = x;
            if x == baseline.len() as isize && y == candidate.len() as isize {
                budget.trace(width)?;
                trace
                    .try_reserve(1)
                    .map_err(|_| PackageSourcePatchError::DiffTraceExceeded {
                        maximum_cells: budget.maximum_trace_cells,
                    })?;
                trace.push(clone_frontier(&frontier, budget.maximum_trace_cells)?);
                return reconstruct_edits(
                    baseline.len(),
                    candidate.len(),
                    &trace,
                    offset,
                    budget.maximum_trace_cells,
                );
            }
            diagonal += 2;
        }
        budget.trace(width)?;
        trace
            .try_reserve(1)
            .map_err(|_| PackageSourcePatchError::DiffTraceExceeded {
                maximum_cells: budget.maximum_trace_cells,
            })?;
        trace.push(clone_frontier(&frontier, budget.maximum_trace_cells)?);
    }
    unreachable!("Myers traversal always reaches the final coordinate")
}

fn clone_frontier(
    frontier: &[isize],
    maximum_trace_cells: usize,
) -> Result<Vec<isize>, PackageSourcePatchError> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(frontier.len()).map_err(|_| {
        PackageSourcePatchError::DiffTraceExceeded {
            maximum_cells: maximum_trace_cells,
        }
    })?;
    cloned.extend_from_slice(frontier);
    Ok(cloned)
}

fn reconstruct_edits(
    baseline_len: usize,
    candidate_len: usize,
    trace: &[Vec<isize>],
    offset: isize,
    maximum_trace_cells: usize,
) -> Result<Vec<Edit>, PackageSourcePatchError> {
    let mut x = baseline_len as isize;
    let mut y = candidate_len as isize;
    let capacity = baseline_len.saturating_add(candidate_len);
    let mut edits = Vec::new();
    edits
        .try_reserve_exact(capacity)
        .map_err(|_| PackageSourcePatchError::DiffTraceExceeded {
            maximum_cells: maximum_trace_cells,
        })?;
    for distance in (1..trace.len()).rev() {
        let prior = &trace[distance - 1];
        let distance = distance as isize;
        let diagonal = x - y;
        let index = usize::try_from(offset + diagonal).expect("trace index is nonnegative");
        let prior_diagonal = if diagonal == -distance
            || (diagonal != distance && prior[index - 1] < prior[index + 1])
        {
            diagonal + 1
        } else {
            diagonal - 1
        };
        let prior_x = prior[(offset + prior_diagonal) as usize];
        let prior_y = prior_x - prior_diagonal;
        while x > prior_x && y > prior_y {
            x -= 1;
            y -= 1;
            edits.push(Edit::Equal {
                baseline: x as usize,
                candidate: y as usize,
            });
        }
        if x == prior_x {
            y -= 1;
            edits.push(Edit::Add {
                candidate: y as usize,
            });
        } else {
            x -= 1;
            edits.push(Edit::Remove {
                baseline: x as usize,
            });
        }
    }
    while x > 0 && y > 0 {
        x -= 1;
        y -= 1;
        edits.push(Edit::Equal {
            baseline: x as usize,
            candidate: y as usize,
        });
    }
    while x > 0 {
        x -= 1;
        edits.push(Edit::Remove {
            baseline: x as usize,
        });
    }
    while y > 0 {
        y -= 1;
        edits.push(Edit::Add {
            candidate: y as usize,
        });
    }
    edits.reverse();
    Ok(edits)
}

fn render_hunks(
    output: &mut BoundedOutput,
    baseline: &[SourceLine<'_>],
    candidate: &[SourceLine<'_>],
    edits: &[Edit],
) -> Result<(), PackageSourcePatchError> {
    let mut ranges = Vec::<(usize, usize)>::new();
    for changed in edits
        .iter()
        .enumerate()
        .filter_map(|(index, edit)| edit.is_change().then_some(index))
    {
        let start = changed.saturating_sub(CONTEXT_LINES);
        let end = edits.len().min(changed.saturating_add(CONTEXT_LINES + 1));
        match ranges.last_mut() {
            Some((_, previous_end)) if start <= *previous_end => *previous_end = end,
            _ => ranges.push((start, end)),
        }
    }

    let mut old_line = 1_usize;
    let mut new_line = 1_usize;
    let mut cursor = 0;
    for (start, end) in ranges {
        while cursor < start {
            advance_line_numbers(edits[cursor], &mut old_line, &mut new_line);
            cursor += 1;
        }
        let old_count = edits[start..end]
            .iter()
            .filter(|edit| !matches!(edit, Edit::Add { .. }))
            .count();
        let new_count = edits[start..end]
            .iter()
            .filter(|edit| !matches!(edit, Edit::Remove { .. }))
            .count();
        output.push("hunk ")?;
        output.push_usize(old_line)?;
        output.push(" ")?;
        output.push_usize(old_count)?;
        output.push(" ")?;
        output.push_usize(new_line)?;
        output.push(" ")?;
        output.push_usize(new_count)?;
        output.push("\n")?;
        while cursor < end {
            match edits[cursor] {
                Edit::Equal {
                    baseline: index, ..
                } => render_source_line(output, "context", baseline[index])?,
                Edit::Remove { baseline: index } => {
                    render_source_line(output, "removed", baseline[index])?
                }
                Edit::Add { candidate: index } => {
                    render_source_line(output, "added", candidate[index])?
                }
            }
            advance_line_numbers(edits[cursor], &mut old_line, &mut new_line);
            cursor += 1;
        }
        output.push("end_hunk\n")?;
    }
    Ok(())
}

fn advance_line_numbers(edit: Edit, baseline: &mut usize, candidate: &mut usize) {
    match edit {
        Edit::Equal { .. } => {
            *baseline += 1;
            *candidate += 1;
        }
        Edit::Remove { .. } => *baseline += 1,
        Edit::Add { .. } => *candidate += 1,
    }
}

fn render_source_line(
    output: &mut BoundedOutput,
    lane: &str,
    line: SourceLine<'_>,
) -> Result<(), PackageSourcePatchError> {
    output.push(lane)?;
    output.push(if line.has_lf { " lf " } else { " none " })?;
    output.push_escaped(line.bytes)?;
    output.push("\n")
}

struct BoundedOutput {
    maximum_bytes: usize,
    rendered: String,
}

impl BoundedOutput {
    fn new(maximum_bytes: usize) -> Self {
        Self {
            maximum_bytes,
            rendered: String::new(),
        }
    }

    fn push(&mut self, value: &str) -> Result<(), PackageSourcePatchError> {
        let required_at_least = self.rendered.len().saturating_add(value.len());
        if required_at_least > self.maximum_bytes {
            return Err(PackageSourcePatchError::OutputExceeded {
                maximum_bytes: self.maximum_bytes,
                required_at_least,
            });
        }
        self.rendered.push_str(value);
        Ok(())
    }

    fn push_usize(&mut self, value: usize) -> Result<(), PackageSourcePatchError> {
        self.push(&value.to_string())
    }

    fn push_hex(&mut self, bytes: &[u8]) -> Result<(), PackageSourcePatchError> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        for byte in bytes {
            let encoded = [
                DIGITS[usize::from(byte >> 4)] as char,
                DIGITS[usize::from(byte & 0x0f)] as char,
            ];
            for digit in encoded {
                let mut buffer = [0_u8; 4];
                self.push(digit.encode_utf8(&mut buffer))?;
            }
        }
        Ok(())
    }

    fn push_escaped(&mut self, bytes: &[u8]) -> Result<(), PackageSourcePatchError> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        for byte in bytes {
            match *byte {
                b'\\' => self.push("\\\\")?,
                0x20..=0x7e => {
                    let literal = [*byte];
                    self.push(std::str::from_utf8(&literal).expect("printable ASCII is UTF-8"))?;
                }
                byte => {
                    let escaped = [
                        b'\\',
                        b'x',
                        DIGITS[usize::from(byte >> 4)],
                        DIGITS[usize::from(byte & 0x0f)],
                    ];
                    self.push(std::str::from_utf8(&escaped).expect("hex escape is ASCII"))?;
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> String {
        self.rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExternalSourceContext, LocalSourceLimits, resolve_external_local_package_source};
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn lines(value: &[u8]) -> Vec<SourceLine<'_>> {
        let count = source_line_count(value);
        split_lines(value, count, count).unwrap()
    }

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-package-source-patch-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn write_package(root: &Path, main: &[u8]) {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(
            root.join("build.omg"),
            b"const PACKAGE: Package = Package { name: \"source-review\" };\n\n\
              machine build(builder: &mut Build) {\n}\n",
        )
        .unwrap();
        std::fs::write(root.join("main.omg"), main).unwrap();
    }

    fn make_tree_writable(root: &Path) {
        let Ok(metadata) = std::fs::symlink_metadata(root) else {
            return;
        };
        if metadata.file_type().is_symlink() {
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = if metadata.is_dir() { 0o700 } else { 0o600 };
            let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(mode));
        }
        #[cfg(not(unix))]
        {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            let _ = std::fs::set_permissions(root, permissions);
        }
        if metadata.is_dir()
            && let Ok(entries) = std::fs::read_dir(root)
        {
            for entry in entries.flatten() {
                make_tree_writable(&entry.path());
            }
        }
    }

    fn cleanup(root: &Path) {
        make_tree_writable(root);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn myers_diff_reconstructs_exact_line_edits() {
        let baseline = lines(b"alpha\nbeta\ngamma\nlast");
        let candidate = lines(b"alpha\nchanged\ngamma\nlast\nadded\n");
        let mut budget = DiffBudget::new(PackageSourcePatchLimits::default());
        budget.add_lines(baseline.len(), candidate.len()).unwrap();
        let edits = myers_diff(&baseline, &candidate, &mut budget).unwrap();
        let mut old = Vec::new();
        let mut new = Vec::new();
        for edit in edits {
            match edit {
                Edit::Equal {
                    baseline: old_index,
                    candidate: new_index,
                } => {
                    old.push(baseline[old_index]);
                    new.push(candidate[new_index]);
                }
                Edit::Remove { baseline: index } => old.push(baseline[index]),
                Edit::Add { candidate: index } => new.push(candidate[index]),
            }
        }
        assert_eq!(old, baseline);
        assert_eq!(new, candidate);
    }

    #[test]
    fn myers_diff_reconstructs_every_small_repeated_line_sequence() {
        fn sequence(mask: usize, length: usize) -> Vec<u8> {
            let mut bytes = Vec::new();
            for index in 0..length {
                bytes.push(if mask & (1 << index) == 0 { b'a' } else { b'b' });
                bytes.push(b'\n');
            }
            bytes
        }

        for baseline_length in 0..=4 {
            for candidate_length in 0..=4 {
                for baseline_mask in 0..(1 << baseline_length) {
                    for candidate_mask in 0..(1 << candidate_length) {
                        let baseline_bytes = sequence(baseline_mask, baseline_length);
                        let candidate_bytes = sequence(candidate_mask, candidate_length);
                        let baseline = lines(&baseline_bytes);
                        let candidate = lines(&candidate_bytes);
                        let mut budget = DiffBudget::new(PackageSourcePatchLimits::default());
                        budget.add_lines(baseline.len(), candidate.len()).unwrap();
                        let edits = myers_diff(&baseline, &candidate, &mut budget).unwrap();
                        let mut reconstructed_baseline = Vec::new();
                        let mut reconstructed_candidate = Vec::new();
                        for edit in edits {
                            match edit {
                                Edit::Equal {
                                    baseline: old_index,
                                    candidate: new_index,
                                } => {
                                    reconstructed_baseline.push(baseline[old_index]);
                                    reconstructed_candidate.push(candidate[new_index]);
                                }
                                Edit::Remove { baseline: index } => {
                                    reconstructed_baseline.push(baseline[index]);
                                }
                                Edit::Add { candidate: index } => {
                                    reconstructed_candidate.push(candidate[index]);
                                }
                            }
                        }
                        assert_eq!(reconstructed_baseline, baseline);
                        assert_eq!(reconstructed_candidate, candidate);
                    }
                }
            }
        }
    }

    #[test]
    fn hunk_rendering_escapes_control_bytes_and_omits_distant_context() {
        let baseline =
            lines(b"same\nold\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\nold-two\n");
        let candidate =
            lines(b"same\nnew\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\nnew-two\x00\n");
        let mut budget = DiffBudget::new(PackageSourcePatchLimits::default());
        budget.add_lines(baseline.len(), candidate.len()).unwrap();
        let edits = myers_diff(&baseline, &candidate, &mut budget).unwrap();
        let mut output = BoundedOutput::new(4_096);
        render_hunks(&mut output, &baseline, &candidate, &edits).unwrap();
        let rendered = output.finish();
        assert!(rendered.contains("removed lf old\nadded lf new\n"));
        assert!(rendered.contains("added lf new-two\\x00\n"));
        assert!(!rendered.contains("context lf six\n"));
        assert_eq!(rendered.matches("hunk ").count(), 2);
    }

    #[test]
    fn entry_rendering_retains_line_endings_modes_kinds_and_symlink_spelling() {
        let baseline_file = VerifiedPackageSourceEntryKind::File {
            bytes: b"first\r\nlast".to_vec(),
            executable: false,
        };
        let candidate_file = VerifiedPackageSourceEntryKind::File {
            bytes: b"first\nlast\n".to_vec(),
            executable: true,
        };
        let mut output = BoundedOutput::new(4_096);
        let mut budget = DiffBudget::new(PackageSourcePatchLimits::default());
        render_entry(
            &mut output,
            &mut budget,
            b"control-\x1b.omg",
            Some(&baseline_file),
            Some(&candidate_file),
        )
        .unwrap();
        let rendered = output.finish();
        assert!(rendered.contains("entry control-\\x1b.omg\n"));
        assert!(rendered.contains("baseline_executable false\n"));
        assert!(rendered.contains("candidate_executable true\n"));
        assert!(rendered.contains("removed lf first\\x0d\n"));
        assert!(rendered.contains("added lf first\n"));
        assert!(rendered.contains("removed none last\n"));
        assert!(rendered.contains("added lf last\n"));

        let directory = VerifiedPackageSourceEntryKind::Directory;
        let symlink = VerifiedPackageSourceEntryKind::Symlink {
            target_bytes: b"../target\nspoof".to_vec(),
        };
        let mut output = BoundedOutput::new(4_096);
        render_entry(
            &mut output,
            &mut budget,
            b"changed-kind",
            Some(&directory),
            Some(&symlink),
        )
        .unwrap();
        let rendered = output.finish();
        assert!(rendered.contains("baseline_kind directory\n"));
        assert!(rendered.contains("candidate_kind symlink\n"));
        assert!(rendered.contains("candidate_target ../target\\x0aspoof\n"));
    }

    #[test]
    fn output_ceiling_rejects_without_returning_a_truncated_patch() {
        let mut output = BoundedOutput::new(5);
        output.push("12345").unwrap();
        assert!(matches!(
            output.push("6"),
            Err(PackageSourcePatchError::OutputExceeded {
                maximum_bytes: 5,
                required_at_least: 6,
            })
        ));
    }

    #[test]
    fn diff_work_and_trace_are_independently_bounded() {
        let baseline = lines(b"a\nb\nc\n");
        let candidate = lines(b"x\ny\nz\n");
        let mut work_budget = DiffBudget::new(PackageSourcePatchLimits::new(
            10, 100, 100, 100, 1, 10_000, 10_000,
        ));
        assert!(matches!(
            myers_diff(&baseline, &candidate, &mut work_budget),
            Err(PackageSourcePatchError::DiffWorkExceeded { maximum: 1 })
        ));

        let mut trace_budget = DiffBudget::new(PackageSourcePatchLimits::new(
            10, 100, 100, 100, 10_000, 1, 10_000,
        ));
        assert!(matches!(
            myers_diff(&baseline, &candidate, &mut trace_budget),
            Err(PackageSourcePatchError::DiffTraceExceeded { maximum_cells: 1 })
        ));
    }

    #[test]
    fn custody_patch_is_exact_bounded_and_marks_unreviewable_content() {
        let live = temp_root("live");
        let baseline_cache = temp_root("baseline-cache");
        let candidate_cache = temp_root("candidate-cache");
        let alternate_cache = temp_root("alternate-cache");
        write_package(&live, b"machine first() {\n}\n");
        let context = ExternalSourceContext::derive(b"source-patch-test");
        let baseline = resolve_external_local_package_source(
            &live,
            &baseline_cache,
            LocalSourceLimits::default(),
            context.clone(),
        )
        .unwrap()
        .into_custody();

        std::fs::write(
            live.join("main.omg"),
            b"machine second() {\n    // end_source_patch\n}\n",
        )
        .unwrap();
        std::fs::write(live.join("opaque.bin"), [0, 0xff, b'\n']).unwrap();
        let candidate = resolve_external_local_package_source(
            &live,
            &candidate_cache,
            LocalSourceLimits::default(),
            context,
        )
        .unwrap()
        .into_custody();

        assert_eq!(baseline.key(), candidate.key());
        let patch = render_package_source_patch(
            Some(&baseline),
            &candidate,
            PackageSourcePatchLimits::default(),
        )
        .unwrap();
        assert_eq!(patch.changed_entries(), 2);
        assert_eq!(patch.incomplete_model_content_entries(), 1);
        assert!(patch.requires_standalone_audit());
        assert!(patch.as_str().contains("removed lf machine first() {"));
        assert!(patch.as_str().contains("added lf machine second() {"));
        assert!(patch.as_str().contains("added lf     // end_source_patch"));
        assert!(
            patch
                .as_str()
                .contains("content_review unavailable_binary_or_non_utf8\n")
        );
        assert_eq!(
            patch
                .as_str()
                .lines()
                .filter(|line| *line == "end_source_patch")
                .count(),
            1,
            "source text cannot forge a renderer control lane"
        );
        for private_root in [&live, &baseline_cache, &candidate_cache] {
            assert!(!patch.as_str().contains(&private_root.display().to_string()));
        }

        let defaults = PackageSourcePatchLimits::default();
        let exact = PackageSourcePatchLimits::new(
            defaults.maximum_entries_per_snapshot(),
            defaults.maximum_bytes_per_snapshot(),
            defaults.maximum_metadata_bytes_per_snapshot(),
            defaults.maximum_lines(),
            defaults.maximum_diff_work(),
            defaults.maximum_trace_cells(),
            patch.as_str().len(),
        );
        assert!(render_package_source_patch(Some(&baseline), &candidate, exact).is_ok());
        let short = PackageSourcePatchLimits::new(
            exact.maximum_entries_per_snapshot(),
            exact.maximum_bytes_per_snapshot(),
            exact.maximum_metadata_bytes_per_snapshot(),
            exact.maximum_lines(),
            exact.maximum_diff_work(),
            exact.maximum_trace_cells(),
            exact.maximum_output_bytes() - 1,
        );
        assert!(matches!(
            render_package_source_patch(Some(&baseline), &candidate, short),
            Err(PackageSourcePatchError::OutputExceeded { .. })
        ));

        let metadata_limited = PackageSourcePatchLimits::new(
            defaults.maximum_entries_per_snapshot(),
            defaults.maximum_bytes_per_snapshot(),
            1,
            defaults.maximum_lines(),
            defaults.maximum_diff_work(),
            defaults.maximum_trace_cells(),
            defaults.maximum_output_bytes(),
        );
        assert!(matches!(
            render_package_source_patch(Some(&baseline), &candidate, metadata_limited),
            Err(PackageSourcePatchError::SourceMetadataExceeded {
                side: PackageSourcePatchSide::Baseline,
                maximum_bytes: 1,
            })
        ));
        let line_limited = PackageSourcePatchLimits::new(
            defaults.maximum_entries_per_snapshot(),
            defaults.maximum_bytes_per_snapshot(),
            defaults.maximum_metadata_bytes_per_snapshot(),
            1,
            defaults.maximum_diff_work(),
            defaults.maximum_trace_cells(),
            defaults.maximum_output_bytes(),
        );
        assert!(matches!(
            render_package_source_patch(Some(&baseline), &candidate, line_limited),
            Err(PackageSourcePatchError::TooManyLines { maximum: 1 })
        ));
        let entry_limited = PackageSourcePatchLimits::new(
            1,
            defaults.maximum_bytes_per_snapshot(),
            defaults.maximum_metadata_bytes_per_snapshot(),
            defaults.maximum_lines(),
            defaults.maximum_diff_work(),
            defaults.maximum_trace_cells(),
            defaults.maximum_output_bytes(),
        );
        assert!(matches!(
            render_package_source_patch(Some(&baseline), &candidate, entry_limited),
            Err(PackageSourcePatchError::SourceCustody {
                side: PackageSourcePatchSide::Baseline,
                error: SourceResolveError::TooManyFiles { limit: 1 },
            })
        ));
        let byte_limited = PackageSourcePatchLimits::new(
            defaults.maximum_entries_per_snapshot(),
            1,
            defaults.maximum_metadata_bytes_per_snapshot(),
            defaults.maximum_lines(),
            defaults.maximum_diff_work(),
            defaults.maximum_trace_cells(),
            defaults.maximum_output_bytes(),
        );
        assert!(matches!(
            render_package_source_patch(Some(&baseline), &candidate, byte_limited),
            Err(PackageSourcePatchError::SourceCustody {
                side: PackageSourcePatchSide::Baseline,
                error: SourceResolveError::TooManyBytes { limit: 1 },
            })
        ));

        let alternate = resolve_external_local_package_source(
            &live,
            &alternate_cache,
            LocalSourceLimits::default(),
            ExternalSourceContext::derive(b"different-context"),
        )
        .unwrap()
        .into_custody();
        assert!(matches!(
            render_package_source_patch(
                Some(&baseline),
                &alternate,
                PackageSourcePatchLimits::default()
            ),
            Err(PackageSourcePatchError::PackageKeyMismatch)
        ));

        cleanup(&live);
        cleanup(&baseline_cache);
        cleanup(&candidate_cache);
        cleanup(&alternate_cache);
    }
}
