//! Bounded line splitting, Myers diff, and hunk construction.

use super::output::{BoundedOutput, render_source_line};
use super::patch::CONTEXT_LINES;
use super::{PackageSourcePatchError, PackageSourcePatchLimits};

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
    pub(super) const fn new(limits: PackageSourcePatchLimits) -> Self {
        Self {
            maximum_lines: limits.maximum_lines(),
            maximum_work: limits.maximum_diff_work(),
            maximum_trace_cells: limits.maximum_trace_cells(),
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
