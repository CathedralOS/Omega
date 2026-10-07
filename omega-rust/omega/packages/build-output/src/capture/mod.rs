//! Capture the private host output tree once, without a sponsor namespace replay.
//!
//! The executor's append-and-seal protocol and private-directory isolation own
//! mutation. Capture owns the actual bounded tree, portable names and immutable
//! bytes. It refuses aliases and special files rather than inventing an object
//! graph to compare against the same invocation's writer. Open-file metadata
//! guards custody during acquisition; content is read once, not rehashed from
//! the host after every stage.

use crate::BuildStagedOutputTree;
use crate::portable_paths::{canonical_relative_path, reserve_path_bytes};
use crate::staged_output_tree::{
    MAX_STAGED_OUTPUT_ENTRIES, MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES, diagnostics, finish_commitment,
};
use diagnostics::Diagnostic;
use filesystem_authority::FilesystemSponsor;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedFileContent {
    pub(crate) executable: bool,
    pub(crate) bytes: Arc<[u8]>,
}
#[derive(Debug)]
pub(crate) enum StagedOutputEntryKind {
    Directory,
    File(CapturedFileContent),
}
#[derive(Debug)]
pub(crate) struct StagedOutputEntry {
    pub(crate) relative_path: Vec<u8>,
    pub(crate) kind: StagedOutputEntryKind,
}

pub fn capture(
    root: &Path,
    sponsor: &FilesystemSponsor,
) -> Result<BuildStagedOutputTree, Vec<Diagnostic>> {
    sponsor.bind_path(root).map_err(|error| {
        diagnostics(format!(
            "cannot bind build staged-output root `{}` to its sponsor: {error}",
            root.display()
        ))
    })?;
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    let mut total_path_bytes = 0usize;
    let mut total_file_bytes = 0u64;
    let limits = sponsor
        .limits()
        .map_err(|error| diagnostics(format!("cannot read staged-output limits: {error}")))?;
    while let Some(directory) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&directory).map_err(|error| {
            diagnostics(format!(
                "cannot inspect build staged-output directory `{}`: {error}",
                directory.display()
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(diagnostics(format!(
                "build staged-output directory `{}` must be a concrete directory",
                directory.display()
            )));
        }
        let children = std::fs::read_dir(&directory).map_err(|error| {
            diagnostics(format!(
                "cannot enumerate build staged-output directory `{}`: {error}",
                directory.display()
            ))
        })?;
        for child in children {
            if entries.len() >= MAX_STAGED_OUTPUT_ENTRIES
                || u64::try_from(entries.len())
                    .map_or(true, |count| count >= limits.maximum_entries)
            {
                return Err(diagnostics(format!(
                    "build staged-output tree exceeds its {}-entry ceiling",
                    limits.maximum_entries.min(MAX_STAGED_OUTPUT_ENTRIES as u64)
                )));
            }
            let child = child.map_err(|error| {
                diagnostics(format!(
                    "cannot enumerate build staged-output entry: {error}"
                ))
            })?;
            let path = child.path();
            let relative = path.strip_prefix(root).map_err(|_| {
                diagnostics(format!(
                    "build staged-output entry `{}` escaped root `{}`",
                    path.display(),
                    root.display()
                ))
            })?;
            let relative_path = canonical_relative_path(relative, &path)?;
            total_path_bytes = reserve_path_bytes(total_path_bytes, relative_path.len())?;
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                diagnostics(format!(
                    "cannot inspect build staged-output entry `{}`: {error}",
                    path.display()
                ))
            })?;
            let kind = if metadata.file_type().is_symlink() {
                return Err(diagnostics(format!(
                    "build staging does not permit symlink entry `{}`",
                    path.display()
                )));
            } else if metadata.is_dir() {
                pending.push(path);
                StagedOutputEntryKind::Directory
            } else if metadata.is_file() {
                let extent = metadata.len();
                if extent > limits.maximum_object_extent {
                    return Err(diagnostics(format!(
                        "build staged-output file `{}` exceeds its {}-byte object ceiling",
                        path.display(),
                        limits.maximum_object_extent
                    )));
                }
                total_file_bytes = total_file_bytes
                    .checked_add(extent)
                    .filter(|total| {
                        *total <= MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES
                            && *total <= limits.maximum_total_logical_bytes
                    })
                    .ok_or_else(|| {
                        diagnostics(format!(
                            "build staged-output tree exceeds its {}-byte ceiling",
                            limits
                                .maximum_total_logical_bytes
                                .min(MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES)
                        ))
                    })?;
                StagedOutputEntryKind::File(capture_file(&path, &relative_path, &metadata, extent)?)
            } else {
                return Err(diagnostics(format!(
                    "build staged-output entry `{}` is not a regular file or directory",
                    path.display()
                )));
            };
            entries.push(StagedOutputEntry {
                relative_path,
                kind,
            });
        }
    }
    Ok(finish_commitment(entries))
}

fn capture_file(
    path: &Path,
    relative_path: &[u8],
    path_metadata: &std::fs::Metadata,
    expected_extent: u64,
) -> Result<CapturedFileContent, Vec<Diagnostic>> {
    let relative_path = String::from_utf8_lossy(relative_path);
    let mut file = std::fs::File::open(path).map_err(|error| {
        diagnostics(format!(
            "cannot open build staged-output file `{}`: {error}",
            relative_path
        ))
    })?;
    let before = file.metadata().map_err(|error| {
        diagnostics(format!(
            "cannot inspect opened build staged-output file `{}`: {error}",
            relative_path
        ))
    })?;
    if !same_file_observation(path_metadata, &before) || before.len() != expected_extent {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed before content capture",
            relative_path
        )));
    }
    if !has_single_link(&before) {
        return Err(diagnostics(format!(
            "build staged-output file `{}` is a hard-link alias or lacks link-count custody",
            relative_path
        )));
    }
    let capacity = usize::try_from(expected_extent).map_err(|_| {
        diagnostics(format!(
            "build staged-output file `{}` is too large for this compiler host",
            relative_path
        ))
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            diagnostics(format!(
                "cannot read build staged-output file `{}`: {error}",
                relative_path
            ))
        })?;
        if count == 0 {
            break;
        }
        let total = bytes
            .len()
            .checked_add(count)
            .filter(|length| *length <= capacity)
            .ok_or_else(|| {
                diagnostics(format!(
                    "build staged-output file `{}` grew during capture",
                    relative_path
                ))
            })?;
        bytes.extend_from_slice(&buffer[..count]);
        debug_assert_eq!(bytes.len(), total);
    }
    let after = file.metadata().map_err(|error| {
        diagnostics(format!(
            "cannot inspect captured build staged-output file `{}`: {error}",
            relative_path
        ))
    })?;
    if bytes.len() != capacity
        || !same_file_observation(&before, &after)
        || !has_single_link(&after)
    {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed during content capture",
            relative_path
        )));
    }
    Ok(CapturedFileContent {
        executable: is_executable(&before),
        bytes: Arc::from(bytes.into_boxed_slice()),
    })
}

#[cfg(unix)]
fn has_single_link(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink() == 1
}
#[cfg(windows)]
fn has_single_link(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.number_of_links() == Some(1)
}
#[cfg(not(any(unix, windows)))]
fn has_single_link(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
pub(crate) fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.mode() & 0o111 != 0
}
#[cfg(not(unix))]
pub(crate) fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
pub(crate) fn same_file_observation(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.is_file()
        && right.is_file()
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mode() == right.mode()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(windows)]
pub(crate) fn same_file_observation(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    left.is_file()
        && right.is_file()
        && left.volume_serial_number().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index().is_some()
        && left.file_index() == right.file_index()
        && left.len() == right.len()
        && left.file_attributes() == right.file_attributes()
        && left.last_write_time() == right.last_write_time()
}
#[cfg(not(any(unix, windows)))]
pub(crate) fn same_file_observation(_left: &std::fs::Metadata, _right: &std::fs::Metadata) -> bool {
    false
}
