//! Capturing what a build wrote from the host filesystem: walking the
//! destination, hashing file content, rejecting hard-link aliases and
//! recording host file identity and executable bits.

use crate::BuildStagedOutputTree;
use crate::portable_paths::{
    canonical_relative_path, canonical_symlink_target, reserve_path_bytes,
};
use crate::staged_output_tree::{
    MAX_STAGED_OUTPUT_ENTRIES, MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES, diagnostics, finish_commitment,
};
use checked_interpreter::{FilesystemSponsor, FilesystemSponsorNamespaceEntryKind};
use diagnostics::Diagnostic;
use sha2::Digest;
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::io::Seek;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedFileContent {
    pub(crate) length: u64,
    pub(crate) digest: [u8; 32],
    pub(crate) executable: bool,
    identity: HostFileIdentity,
    pub(crate) bytes: Arc<[u8]>,
}

#[derive(Debug)]
pub(crate) enum StagedOutputEntryKind {
    Directory,
    File(CapturedFileContent),
    Symlink { target: Vec<u8> },
}

#[derive(Debug)]
pub(crate) struct StagedOutputEntry {
    pub(crate) relative_path: Vec<u8>,
    pub(crate) kind: StagedOutputEntryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedEntryKind {
    Directory,
    File { group: u64, extent: u64 },
    Symlink { spelling_bytes: u64 },
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostFileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(not(unix))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostFileIdentity;

pub fn capture(
    root: &Path,
    sponsor: &FilesystemSponsor,
) -> Result<BuildStagedOutputTree, Vec<Diagnostic>> {
    let snapshot = sponsor.namespace_snapshot().map_err(|error| {
        diagnostics(format!(
            "cannot inspect sponsored build staged-output namespace: {error}"
        ))
    })?;
    if snapshot.transaction_prepared() || snapshot.open_descriptors() != 0 {
        return Err(diagnostics(
            "build staged-output capture requires a quiescent sponsor with no prepared transaction or open descriptor",
        ));
    }
    let bound_root = sponsor.bind_path(root).map_err(|error| {
        diagnostics(format!(
            "cannot bind build staged-output root `{}` to its sponsor: {error}",
            root.display()
        ))
    })?;
    let sponsor_root = bound_root.relative();
    let mut root_is_sponsored_directory = false;
    let mut expected = BTreeMap::new();
    for entry in snapshot.entries() {
        if entry.relative_path() == sponsor_root {
            root_is_sponsored_directory =
                entry.kind() == FilesystemSponsorNamespaceEntryKind::Directory;
            continue;
        }
        let Ok(relative) = entry.relative_path().strip_prefix(sponsor_root) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let kind = match entry.kind() {
            FilesystemSponsorNamespaceEntryKind::Directory => ExpectedEntryKind::Directory,
            FilesystemSponsorNamespaceEntryKind::Symlink { spelling_bytes } => {
                ExpectedEntryKind::Symlink { spelling_bytes }
            }
            FilesystemSponsorNamespaceEntryKind::Object { group, extent } => {
                ExpectedEntryKind::File { group, extent }
            }
        };
        expected.insert(relative.to_path_buf(), kind);
    }
    if !root_is_sponsored_directory {
        return Err(diagnostics(format!(
            "build staged-output root `{}` is not the sponsor's committed directory",
            root.display()
        )));
    }
    if expected.len() > MAX_STAGED_OUTPUT_ENTRIES {
        return Err(diagnostics(format!(
            "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_ENTRIES}-entry ceiling"
        )));
    }

    let metadata = std::fs::symlink_metadata(root).map_err(|error| {
        diagnostics(format!(
            "cannot inspect build staged-output root `{}`: {error}",
            root.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(diagnostics(format!(
            "build staged-output root `{}` must be a concrete directory",
            root.display()
        )));
    }

    let mut observed_paths = BTreeSet::new();
    let mut entries = Vec::with_capacity(expected.len());
    let mut pending = vec![root.to_path_buf()];
    let mut total_path_bytes = 0usize;
    let mut total_unique_file_bytes = 0u64;
    let mut file_groups = BTreeMap::<u64, CapturedFileContent>::new();
    while let Some(directory) = pending.pop() {
        let directory_metadata = std::fs::symlink_metadata(&directory).map_err(|error| {
            diagnostics(format!(
                "cannot inspect build staged-output directory `{}`: {error}",
                directory.display()
            ))
        })?;
        if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
            return Err(diagnostics(format!(
                "build staged-output directory `{}` changed kind during capture",
                directory.display()
            )));
        }
        let children = std::fs::read_dir(&directory).map_err(|error| {
            diagnostics(format!(
                "cannot enumerate build staged-output directory `{}`: {error}",
                directory.display()
            ))
        })?;
        let mut bounded_children = Vec::new();
        for child in children {
            if observed_paths.len() + bounded_children.len() == MAX_STAGED_OUTPUT_ENTRIES {
                return Err(diagnostics(format!(
                    "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_ENTRIES}-entry ceiling"
                )));
            }
            bounded_children.push(child.map_err(|error| {
                diagnostics(format!(
                    "cannot enumerate build staged-output directory `{}`: {error}",
                    directory.display()
                ))
            })?);
        }
        bounded_children.sort_by_key(|left| left.file_name());

        for child in bounded_children {
            let path = child.path();
            let relative_native = path.strip_prefix(root).map_err(|_| {
                diagnostics(format!(
                    "build staged-output entry `{}` escaped root `{}`",
                    path.display(),
                    root.display()
                ))
            })?;
            let expected_kind = expected.get(relative_native).copied().ok_or_else(|| {
                diagnostics(format!(
                    "build staged-output entry `{}` is absent from sponsor custody",
                    path.display()
                ))
            })?;
            if !observed_paths.insert(relative_native.to_path_buf()) {
                return Err(diagnostics(format!(
                    "build staged-output entry `{}` was observed more than once",
                    path.display()
                )));
            }
            let relative_path = canonical_relative_path(relative_native, &path)?;
            total_path_bytes = reserve_path_bytes(total_path_bytes, relative_path.len())?;
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                diagnostics(format!(
                    "cannot inspect build staged-output entry `{}`: {error}",
                    path.display()
                ))
            })?;
            let file_type = metadata.file_type();
            let kind = match expected_kind {
                ExpectedEntryKind::Directory if file_type.is_dir() => {
                    pending.push(path);
                    StagedOutputEntryKind::Directory
                }
                ExpectedEntryKind::File { group, extent } if file_type.is_file() => {
                    if metadata.len() != extent {
                        return Err(diagnostics(format!(
                            "build staged-output file `{}` disagrees with sponsor extent",
                            path.display()
                        )));
                    }
                    match file_groups.get(&group) {
                        Some(existing) => {
                            validate_hard_link_alias(&path, &metadata, extent, existing)?;
                            StagedOutputEntryKind::File(existing.clone())
                        }
                        None => {
                            total_unique_file_bytes = total_unique_file_bytes
                                .checked_add(extent)
                                .filter(|total| *total <= MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES)
                                .ok_or_else(|| {
                                    diagnostics(format!(
                                        "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES}-byte unique-content ceiling"
                                    ))
                                })?;
                            let content = capture_file(&path, &metadata, extent)?;
                            file_groups.insert(group, content.clone());
                            StagedOutputEntryKind::File(content)
                        }
                    }
                }
                ExpectedEntryKind::Symlink { spelling_bytes } if file_type.is_symlink() => {
                    let target = std::fs::read_link(&path).map_err(|error| {
                        diagnostics(format!(
                            "cannot read build staged-output symlink `{}`: {error}",
                            path.display()
                        ))
                    })?;
                    let target = canonical_symlink_target(&target, &relative_path, &path)?;
                    if u64::try_from(target.len()).ok() != Some(spelling_bytes) {
                        return Err(diagnostics(format!(
                            "build staged-output symlink `{}` disagrees with sponsor target length",
                            path.display()
                        )));
                    }
                    total_path_bytes = reserve_path_bytes(total_path_bytes, target.len())?;
                    StagedOutputEntryKind::Symlink { target }
                }
                _ => {
                    return Err(diagnostics(format!(
                        "build staged-output entry `{}` disagrees with sponsor kind",
                        path.display()
                    )));
                }
            };
            entries.push(StagedOutputEntry {
                relative_path,
                kind,
            });
        }
    }
    if observed_paths.len() != expected.len() {
        let missing = expected
            .keys()
            .find(|path| !observed_paths.contains(*path))
            .expect("unequal sponsored and observed counts have one missing path");
        return Err(diagnostics(format!(
            "sponsored build staged-output entry `{}` is missing from the physical tree",
            root.join(missing).display()
        )));
    }
    Ok(finish_commitment(entries))
}

fn capture_file(
    path: &Path,
    path_metadata: &std::fs::Metadata,
    expected_extent: u64,
) -> Result<CapturedFileContent, Vec<Diagnostic>> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        diagnostics(format!(
            "cannot open build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    let before = file.metadata().map_err(|error| {
        diagnostics(format!(
            "cannot inspect opened build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    if !same_file_observation(path_metadata, &before) || before.len() != expected_extent {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed before content capture",
            path.display()
        )));
    }
    let (first, bytes) = capture_reader(&mut file, expected_extent, path)?;
    file.rewind().map_err(|error| {
        diagnostics(format!(
            "cannot rewind build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    let second = hash_reader(&mut file, expected_extent, path)?;
    let after = file.metadata().map_err(|error| {
        diagnostics(format!(
            "cannot re-inspect build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    if first != second || !same_file_observation(&before, &after) {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed while content was captured",
            path.display()
        )));
    }
    Ok(CapturedFileContent {
        length: expected_extent,
        digest: first,
        executable: is_executable(&before),
        identity: host_file_identity(&before),
        bytes,
    })
}

fn capture_reader(
    reader: &mut std::fs::File,
    expected_extent: u64,
    path: &Path,
) -> Result<([u8; 32], Arc<[u8]>), Vec<Diagnostic>> {
    let capacity = usize::try_from(expected_extent).map_err(|_| {
        diagnostics(format!(
            "build staged-output file `{}` is too large to retain on this compiler host",
            path.display()
        ))
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut digest = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|error| {
            diagnostics(format!(
                "cannot read build staged-output file `{}`: {error}",
                path.display()
            ))
        })?;
        if read == 0 {
            break;
        }
        total = total.checked_add(read as u64).ok_or_else(|| {
            diagnostics(format!(
                "build staged-output file `{}` length overflowed during capture",
                path.display()
            ))
        })?;
        if total > expected_extent {
            return Err(diagnostics(format!(
                "build staged-output file `{}` grew during capture",
                path.display()
            )));
        }
        digest.update(&buffer[..read]);
        bytes.extend_from_slice(&buffer[..read]);
    }
    if total != expected_extent {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed length during capture",
            path.display()
        )));
    }
    Ok((
        digest.finalize().into(),
        Arc::from(bytes.into_boxed_slice()),
    ))
}

#[cfg(unix)]
fn validate_hard_link_alias(
    path: &Path,
    metadata: &std::fs::Metadata,
    expected_extent: u64,
    expected: &CapturedFileContent,
) -> Result<(), Vec<Diagnostic>> {
    if metadata.len() != expected_extent
        || host_file_identity(metadata) != expected.identity
        || is_executable(metadata) != expected.executable
    {
        return Err(diagnostics(format!(
            "build staged-output hard-link group disagrees at `{}`",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_hard_link_alias(
    path: &Path,
    metadata: &std::fs::Metadata,
    expected_extent: u64,
    expected: &CapturedFileContent,
) -> Result<(), Vec<Diagnostic>> {
    let observed = capture_file(path, metadata, expected_extent)?;
    if &observed != expected {
        return Err(diagnostics(format!(
            "build staged-output hard-link group disagrees at `{}`",
            path.display()
        )));
    }
    Ok(())
}

fn hash_reader(
    reader: &mut std::fs::File,
    expected_extent: u64,
    path: &Path,
) -> Result<[u8; 32], Vec<Diagnostic>> {
    let mut digest = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|error| {
            diagnostics(format!(
                "cannot read build staged-output file `{}`: {error}",
                path.display()
            ))
        })?;
        if read == 0 {
            break;
        }
        total = total.checked_add(read as u64).ok_or_else(|| {
            diagnostics(format!(
                "build staged-output file `{}` length overflowed during capture",
                path.display()
            ))
        })?;
        if total > expected_extent {
            return Err(diagnostics(format!(
                "build staged-output file `{}` grew during capture",
                path.display()
            )));
        }
        digest.update(&buffer[..read]);
    }
    if total != expected_extent {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed length during capture",
            path.display()
        )));
    }
    Ok(digest.finalize().into())
}

#[cfg(unix)]
fn host_file_identity(metadata: &std::fs::Metadata) -> HostFileIdentity {
    use std::os::unix::fs::MetadataExt;
    HostFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(not(unix))]
fn host_file_identity(_metadata: &std::fs::Metadata) -> HostFileIdentity {
    HostFileIdentity
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
    left.file_type().is_file()
        && right.file_type().is_file()
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mode() == right.mode()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
pub(crate) fn same_file_observation(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    left.file_type().is_file()
        && right.file_type().is_file()
        && left.len() == right.len()
        && left.permissions().readonly() == right.permissions().readonly()
        && left.modified().ok() == right.modified().ok()
}
