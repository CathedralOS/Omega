//! Bounded, hostile-data-safe patches for source review.

mod diff;
mod output;
mod patch;
mod snapshot;

pub(crate) use patch::{
    PackageSourcePatchError, PackageSourcePatchSide, render_package_source_patch,
};

/// Independent ceilings for one model-facing source patch.
///
/// Snapshot bytes are capped before capture, diff work is counted across all
/// changed files, and output rejects rather than truncates. These are review
/// resource limits, not package identity or admission policy; no caller varies
/// them, so the renderer owns them rather than threading a limits record.
const MAXIMUM_ENTRIES_PER_SNAPSHOT: usize = 4_096;
const MAXIMUM_BYTES_PER_SNAPSHOT: u64 = 16 * 1024 * 1024;
const MAXIMUM_METADATA_BYTES_PER_SNAPSHOT: usize = 4 * 1024 * 1024;
const MAXIMUM_LINES: usize = 250_000;
const MAXIMUM_DIFF_WORK: usize = 4_000_000;
const MAXIMUM_TRACE_CELLS: usize = 4_000_000;
const MAXIMUM_OUTPUT_BYTES: usize = 1024 * 1024;
