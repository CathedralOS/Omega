//! Both retained bytes and metadata-only commitments use the same checked
//! read. Path metadata alone misses substitution during open; length alone
//! misses in-place edits. Compare the inspected file, opened handle, completed
//! handle, and final path before publishing either result.
//!
//! Host identities and times are drift guards, never canonical input metadata.
//! They detect ordinary concurrent edits under resolver-owned backing custody,
//! not arbitrary same-user interference or atomicity across a mutable tree.

use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::Read;
use std::path::Path;

pub(super) fn read_canonical_source_file(
    path: &Path,
    initial_metadata: &Metadata,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let file_length = usize::try_from(initial_metadata.len())
        .map_err(|_| "canonical Source file length exceeds usize".to_owned())?;
    bytes
        .try_reserve_exact(file_length)
        .map_err(|_| "canonical Source file allocation failed on this compiler host".to_owned())?;
    read_canonical_file_chunks(path, initial_metadata, |chunk| {
        bytes.extend_from_slice(chunk)
    })?;
    Ok(bytes)
}

pub(super) fn hash_canonical_source_file(
    path: &Path,
    initial_metadata: &Metadata,
) -> Result<[u8; 32], String> {
    let mut digest = Sha256::new();
    read_canonical_file_chunks(path, initial_metadata, |chunk| digest.update(chunk))?;
    Ok(digest.finalize().into())
}

/// Chunk consumers only accumulate private results: none may publish before
/// the final handle and path checks succeed.
pub(super) fn read_canonical_file_chunks(
    path: &Path,
    initial_metadata: &Metadata,
    mut consume_chunk: impl FnMut(&[u8]),
) -> Result<(), String> {
    let mut file = File::open(path).map_err(|error| {
        format!(
            "could not open canonical Source file {}: {error}",
            path.display()
        )
    })?;
    let opened_metadata = file.metadata().map_err(|error| {
        format!(
            "could not inspect opened Source file {}: {error}",
            path.display()
        )
    })?;
    require_unchanged_file(path, initial_metadata, &opened_metadata)?;

    let mut observed = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            format!(
                "could not read canonical Source file {}: {error}",
                path.display()
            )
        })?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(count as u64)
            .filter(|observed| *observed <= initial_metadata.len())
            .ok_or_else(|| {
                format!(
                    "canonical Source file {} grew while captured",
                    path.display()
                )
            })?;
        consume_chunk(&buffer[..count]);
    }
    if observed != initial_metadata.len() {
        return Err(format!(
            "canonical Source file {} changed length while captured",
            path.display()
        ));
    }
    let completed_metadata = file.metadata().map_err(|error| {
        format!(
            "could not recheck opened Source file {}: {error}",
            path.display()
        )
    })?;
    require_unchanged_file(path, &opened_metadata, &completed_metadata)?;
    let final_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!(
            "could not recheck canonical Source file {}: {error}",
            path.display()
        )
    })?;
    require_unchanged_file(path, &completed_metadata, &final_metadata)
}

fn require_unchanged_file(
    path: &Path,
    expected: &Metadata,
    observed: &Metadata,
) -> Result<(), String> {
    let expected_modified = expected.modified().map_err(|error| {
        format!(
            "could not inspect Source modification time {}: {error}",
            path.display()
        )
    })?;
    let observed_modified = observed.modified().map_err(|error| {
        format!(
            "could not recheck Source modification time {}: {error}",
            path.display()
        )
    })?;
    let unchanged = expected.is_file()
        && observed.is_file()
        && expected.len() == observed.len()
        && expected.permissions() == observed.permissions()
        && expected_modified == observed_modified;
    #[cfg(unix)]
    let unchanged = {
        use std::os::unix::fs::MetadataExt;
        unchanged
            && expected.dev() == observed.dev()
            && expected.ino() == observed.ino()
            && expected.ctime() == observed.ctime()
            && expected.ctime_nsec() == observed.ctime_nsec()
    };
    #[cfg(windows)]
    let unchanged = {
        use std::os::windows::fs::MetadataExt;
        // Additional drift indicators, not a unique file identity. These
        // checks do not replace the caller's backing-store custody.
        unchanged
            && expected.creation_time() == observed.creation_time()
            && expected.file_attributes() == observed.file_attributes()
    };
    if unchanged {
        Ok(())
    } else {
        Err(format!(
            "canonical Source file {} changed identity or content while captured",
            path.display()
        ))
    }
}
