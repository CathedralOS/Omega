/*
    We render bounded, content-verified source patches (`PackageSourcePatch`)
    between an accepted baseline snapshot and a candidate package snapshot (or a
    standalone candidate review when no baseline snapshot is available). Both
    snapshots are verified against their resolver-issued content commitments
    during capture and revalidated after rendering.

    The fixed grammar and metadata are trusted renderer output, whereas source
    paths, symlink payloads, and line bytes remain untrusted package data that
    are lane-prefixed and byte-escaped (`BoundedOutput::push_escaped`). Line
    splitting, Myers shortest-edit-script diffing (`myers_diff`), trace storage,
    and output bytes are strictly bounded by `DiffBudget` and `BoundedOutput`
    ceilings.
*/

/*
    Audit source-diff rendering belongs to the package review audit boundary
    (`review/audit/mod.rs`).
*/

mod snapshot;

use crate::Hex;
use crate::resolution::source::{PackageSourceCustody, PackageSourceSelectionEvidenceError};
use crate::review::timings;
use package_source::SourceResolveError;
use snapshot::{
    capture_snapshot, file_content_requires_standalone_audit, render_entry, render_resolution,
    revalidate_snapshot, side_token,
};
use std::collections::BTreeSet;
use std::fmt;

pub(crate) const MAXIMUM_ENTRIES_PER_SNAPSHOT: usize = 4096;
pub(crate) const MAXIMUM_BYTES_PER_SNAPSHOT: u64 = 16 * 1024 * 1024;
pub(crate) const MAXIMUM_METADATA_BYTES_PER_SNAPSHOT: usize = 1024 * 1024;
pub(crate) const MAXIMUM_LINES: usize = 65_536;
pub(crate) const MAXIMUM_DIFF_WORK: usize = 8_000_000;
pub(crate) const MAXIMUM_TRACE_CELLS: usize = 16_000_000;
pub(crate) const MAXIMUM_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageSourcePatch {
    changed_entries: usize,
    incomplete_model_content_entries: usize,
    rendered: String,
}

impl PackageSourcePatch {
    pub const fn changed_entries(&self) -> usize {
        self.changed_entries
    }

    pub const fn requires_standalone_audit(&self) -> bool {
        self.incomplete_model_content_entries != 0
    }

    pub fn as_str(&self) -> &str {
        &self.rendered
    }
}

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
        Some(baseline) => output.push(&Hex(&baseline.key().identity().digest()).to_string())?,
        None => output.push("none")?,
    }
    output.push("\n")?;
    output.push("candidate_key ")?;
    output.push(&Hex(&candidate.key().identity().digest()).to_string())?;
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
        changed_entries,
        incomplete_model_content_entries,
        rendered: output.finish(),
    })
}

pub(super) fn render_source_line(
    output: &mut BoundedOutput,
    lane: &str,
    line: SourceLine<'_>,
) -> Result<(), PackageSourcePatchError> {
    output.push(lane)?;
    output.push(if line.has_lf { " lf " } else { " none " })?;
    output.push_escaped(line.bytes)?;
    output.push("\n")
}

pub(super) struct BoundedOutput {
    maximum_bytes: usize,
    rendered: String,
}

impl BoundedOutput {
    pub(super) fn new(maximum_bytes: usize) -> Self {
        Self {
            maximum_bytes,
            rendered: String::new(),
        }
    }

    pub(super) fn push(&mut self, value: &str) -> Result<(), PackageSourcePatchError> {
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

    pub(super) fn push_usize(&mut self, value: usize) -> Result<(), PackageSourcePatchError> {
        self.push(&value.to_string())
    }

    pub(super) fn push_escaped(&mut self, bytes: &[u8]) -> Result<(), PackageSourcePatchError> {
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

    pub(super) fn finish(self) -> String {
        self.rendered
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceLine<'source> {
    pub(super) bytes: &'source [u8],
    pub(super) has_lf: bool,
}

pub(super) fn source_line_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count()
        + usize::from(!bytes.is_empty() && !bytes.ends_with(b"\n"))
}

pub(super) fn split_lines(
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
pub(super) enum Edit {
    Equal { baseline: usize, candidate: usize },
    Remove { baseline: usize },
    Add { candidate: usize },
}

impl Edit {
    const fn is_change(self) -> bool {
        !matches!(self, Self::Equal { .. })
    }
}

pub(super) struct DiffBudget {
    pub(super) maximum_lines: usize,
    maximum_work: usize,
    maximum_trace_cells: usize,
    lines: usize,
    work: usize,
    trace_cells: usize,
}

impl DiffBudget {
    pub(super) const fn new() -> Self {
        Self {
            maximum_lines: MAXIMUM_LINES,
            maximum_work: MAXIMUM_DIFF_WORK,
            maximum_trace_cells: MAXIMUM_TRACE_CELLS,
            lines: 0,
            work: 0,
            trace_cells: 0,
        }
    }

    pub(super) fn add_lines(
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

    pub(super) fn work(&mut self) -> Result<(), PackageSourcePatchError> {
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

    pub(super) fn trace(&mut self, cells: usize) -> Result<(), PackageSourcePatchError> {
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

pub(super) fn myers_diff(
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

pub(super) fn render_hunks(
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
