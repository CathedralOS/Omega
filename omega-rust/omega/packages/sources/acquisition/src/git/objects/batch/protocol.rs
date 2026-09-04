//! Exact request encoding, response bounds, and blob assignment.

use std::sync::Arc;

use crate::error::{SourceResolveError, git_tree_invalid};
use crate::limits::LocalSourceLimits;

use super::super::tree::validate_git_symlink_target;
use super::super::{GitBlobBytes, GitTreeEntry, GitTreeEntryKind};

pub(super) fn git_batch_request_bytes(entries: &[GitTreeEntry]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        bytes.extend_from_slice(entry.oid.as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

pub(crate) fn git_batch_output_limit(
    entries: &[GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<usize, SourceResolveError> {
    let mut payload_bytes = 0_u64;
    let mut output_bytes = 0_usize;
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        payload_bytes =
            payload_bytes
                .checked_add(entry.size)
                .ok_or(SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                })?;
        if payload_bytes > limits.max_bytes {
            return Err(SourceResolveError::TooManyBytes {
                limit: limits.max_bytes,
            });
        }
        let size = usize::try_from(entry.size).map_err(|_| {
            git_tree_invalid(entry.oid.as_bytes(), "blob cannot fit in host memory")
        })?;
        output_bytes = output_bytes
            .checked_add(entry.oid.len())
            .and_then(|value| value.checked_add(b" blob ".len()))
            .and_then(|value| value.checked_add(decimal_digit_count(entry.size)))
            .and_then(|value| value.checked_add(1))
            .and_then(|value| value.checked_add(size))
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| {
                git_tree_invalid(
                    entry.oid.as_bytes(),
                    "batch output cannot fit in host memory",
                )
            })?;
    }
    Ok(output_bytes)
}

fn decimal_digit_count(mut value: u64) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

pub(crate) fn assign_git_batch_output(
    entries: &mut [GitTreeEntry],
    output: Vec<u8>,
) -> Result<(), SourceResolveError> {
    let mut remaining = output.as_slice();
    let mut offset = 0_usize;
    let mut ranges = Vec::with_capacity(entries.len());
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        let Some(header_end) = remaining.iter().position(|byte| *byte == b'\n') else {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "truncated cat-file batch header",
            ));
        };
        let header = &remaining[..=header_end];
        let expected_header = format!("{} blob {}\n", entry.oid, entry.size);
        if header != expected_header.as_bytes() {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "cat-file batch header did not match the exact requested blob",
            ));
        }
        remaining = &remaining[header_end + 1..];
        offset = offset
            .checked_add(header_end + 1)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
        let size = usize::try_from(entry.size).map_err(|_| {
            git_tree_invalid(entry.oid.as_bytes(), "blob cannot fit in host memory")
        })?;
        let Some(bytes) = remaining.get(..size) else {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "truncated cat-file batch blob",
            ));
        };
        if remaining.get(size) != Some(&b'\n') {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "cat-file batch blob lacks its separator",
            ));
        }
        if matches!(&entry.kind, GitTreeEntryKind::Symlink { .. }) {
            validate_git_symlink_target(&entry.relative_bytes, bytes)?;
        }
        let end = offset
            .checked_add(size)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
        ranges.push(offset..end);
        remaining = &remaining[size + 1..];
        offset = end
            .checked_add(1)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
    }
    if !remaining.is_empty() {
        return Err(git_tree_invalid(
            Vec::new(),
            "cat-file batch returned an unexpected trailing response",
        ));
    }
    let batch = Arc::new(output);
    for (entry, range) in entries
        .iter_mut()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
        .zip(ranges)
    {
        match &mut entry.kind {
            GitTreeEntryKind::Tree => unreachable!("tree rows are excluded from blob assignment"),
            GitTreeEntryKind::File { bytes, .. } => {
                *bytes = GitBlobBytes {
                    batch: Arc::clone(&batch),
                    start: range.start,
                    end: range.end,
                };
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                *target_bytes = GitBlobBytes {
                    batch: Arc::clone(&batch),
                    start: range.start,
                    end: range.end,
                };
            }
        }
    }
    Ok(())
}
