//! Capability-relative local tree capture and canonical content identity.

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{
    ambient_authority,
    fs::{Dir as CapabilityDirectory, OpenOptions as CapabilityOpenOptions},
};
use sha2::{Digest, Sha256};

use super::model::ResolvedLocalSource;
use crate::source::SourceResolveError;
use crate::source::git::process::identity::format_sha256;
use crate::source::limits::{
    CANONICAL_DIRECTORY_MODE, DEFAULT_BUILD_OUTPUT_DIRECTORY, LocalSourceLimits,
};

#[derive(Debug)]
struct SourceEntry {
    relative_bytes: Vec<u8>,
    relative_path: PathBuf,
    kind: SourceEntryKind,
}

#[derive(Debug)]
enum SourceEntryKind {
    Directory,
    File { bytes: Vec<u8>, executable: bool },
    Symlink { target_bytes: Vec<u8> },
}

#[derive(Debug)]
pub(in crate::source) struct CapturedLocalTree {
    pub(in crate::source) normalized: ResolvedLocalSource,
    pub(in crate::source) entries: Vec<CapturedLocalEntry>,
}

#[derive(Debug)]
pub(in crate::source) struct CapturedLocalEntry {
    pub(in crate::source) relative_path: PathBuf,
    pub(in crate::source) relative_bytes: Vec<u8>,
    pub(in crate::source) kind: CapturedLocalEntryKind,
}

#[derive(Debug)]
pub(in crate::source) enum CapturedLocalEntryKind {
    Directory,
    File { bytes: Vec<u8>, executable: bool },
    Symlink { target_bytes: Vec<u8> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::source) enum SourceTreePolicy {
    /// Mutable local package roots omit only paths reserved for resolver or compiler output.
    LocalPackage,
    /// Resolver-owned materializations must be hashed exactly as published.
    ExactMaterialized,
}

#[cfg(test)]
pub(in crate::source) fn resolve_materialized_source(
    root: &Path,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    Ok(capture_local_source(root, limits, SourceTreePolicy::ExactMaterialized)?.normalized)
}

pub(in crate::source) fn capture_local_source(
    requested_root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
) -> Result<CapturedLocalTree, SourceResolveError> {
    let root = requested_root
        .canonicalize()
        .map_err(|error| io_error(requested_root, error))?;
    if !root.is_dir() {
        return Err(SourceResolveError::NotDirectory { path: root });
    }

    let root_directory = open_canonical_source_root(&root)?;
    capture_local_source_from_open_root(root, root_directory, limits, policy)
}

pub(in crate::source) fn open_canonical_source_root(
    canonical_root: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let directory = open_absolute_directory_nofollow(canonical_root)
        .map_err(|error| io_error(canonical_root, error))?;
    let metadata = directory
        .dir_metadata()
        .map_err(|error| io_error(canonical_root, error))?;
    if !metadata.is_dir() {
        return Err(SourceResolveError::NotDirectory {
            path: canonical_root.to_path_buf(),
        });
    }
    Ok(directory)
}

pub(in crate::source) fn open_absolute_directory_nofollow(
    canonical_root: &Path,
) -> Result<CapabilityDirectory, std::io::Error> {
    use std::path::Component;

    let mut anchor = PathBuf::new();
    let mut relative_components = Vec::new();
    for component in canonical_root.components() {
        match component {
            Component::Prefix(prefix) => anchor.push(prefix.as_os_str()),
            Component::RootDir => anchor.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(name) => relative_components.push(name.to_os_string()),
            Component::ParentDir => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "canonical directory contains a parent component",
                ));
            }
        }
    }
    if anchor.as_os_str().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "canonical directory is not absolute",
        ));
    }

    let mut directory = CapabilityDirectory::open_ambient_dir(&anchor, ambient_authority())?;
    for component in relative_components {
        directory = directory.open_dir_nofollow(&component)?;
    }
    let metadata = directory.dir_metadata()?;
    if !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "opened path is not a directory",
        ));
    }
    Ok(directory)
}

pub(in crate::source) fn capture_local_source_from_open_root(
    root: PathBuf,
    root_directory: CapabilityDirectory,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
) -> Result<CapturedLocalTree, SourceResolveError> {
    let mut source_entries = Vec::new();
    let mut captured_file_bytes = 0_u64;
    visit_directory(
        &root_directory,
        &root_directory,
        &root,
        PathBuf::new(),
        0,
        &root,
        limits,
        policy,
        &mut captured_file_bytes,
        &mut source_entries,
    )?;
    source_entries.sort_by(|left, right| left.relative_bytes.cmp(&right.relative_bytes));

    let mut identity = SourceIdentityHasher::new(source_entries.len());
    let mut file_count = 0;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(source_entries.len())
        .map_err(|_| SourceResolveError::TooManyFiles {
            limit: limits.max_files,
        })?;
    for entry in source_entries {
        let kind = match entry.kind {
            SourceEntryKind::Directory => {
                identity.add_directory(&entry.relative_bytes, CANONICAL_DIRECTORY_MODE);
                CapturedLocalEntryKind::Directory
            }
            SourceEntryKind::File { bytes, executable } => {
                identity.add_file(&entry.relative_bytes, executable, &bytes)?;
                file_count += 1;
                CapturedLocalEntryKind::File { bytes, executable }
            }
            SourceEntryKind::Symlink { target_bytes } => {
                identity.add_symlink(&entry.relative_bytes, &target_bytes);
                file_count += 1;
                CapturedLocalEntryKind::Symlink { target_bytes }
            }
        };
        entries.push(CapturedLocalEntry {
            relative_path: entry.relative_path,
            relative_bytes: entry.relative_bytes,
            kind,
        });
    }
    let (byte_count, content_identity) = identity.finish();
    Ok(CapturedLocalTree {
        normalized: ResolvedLocalSource {
            root,
            file_count,
            byte_count,
            content_identity,
        },
        entries,
    })
}

fn visit_directory(
    root_directory: &CapabilityDirectory,
    directory: &CapabilityDirectory,
    display_dir: &Path,
    logical_dir: PathBuf,
    depth: usize,
    root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
    captured_file_bytes: &mut u64,
    entries: &mut Vec<SourceEntry>,
) -> Result<(), SourceResolveError> {
    if depth > limits.max_depth {
        return Err(SourceResolveError::TooDeep {
            path: display_dir.to_path_buf(),
            limit: limits.max_depth,
        });
    }

    let remaining_entries = limits.max_files.saturating_sub(entries.len());
    let excluded_entry_allowance = match policy {
        SourceTreePolicy::ExactMaterialized => 0,
        SourceTreePolicy::LocalPackage if logical_dir.as_os_str().is_empty() => 2,
        SourceTreePolicy::LocalPackage => 1,
    };
    let directory_listing_limit = remaining_entries.saturating_add(excluded_entry_allowance);
    let mut entry_names = Vec::new();
    for entry in directory
        .entries()
        .map_err(|error| io_error(display_dir, error))?
    {
        if entry_names.len() >= directory_listing_limit {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_files,
            });
        }
        entry_names.push(
            entry
                .map_err(|error| io_error(display_dir, error))?
                .file_name(),
        );
    }
    entry_names.sort();

    for name in entry_names {
        if policy == SourceTreePolicy::LocalPackage
            && (name == ".git"
                || (logical_dir.as_os_str().is_empty() && name == DEFAULT_BUILD_OUTPUT_DIRECTORY))
        {
            continue;
        }
        if entries.len() >= limits.max_files {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_files,
            });
        }
        let display_path = display_dir.join(&name);
        let logical_path = logical_dir.join(&name);
        let metadata = directory
            .symlink_metadata(&name)
            .map_err(|error| io_error(&display_path, error))?;
        if metadata.file_type().is_symlink() {
            let raw_target = read_and_validate_symlink_target(
                root_directory,
                root,
                directory,
                &logical_dir,
                &name,
                &display_path,
                policy,
            )?;
            push_entry(
                entries,
                logical_path,
                SourceEntryKind::Symlink {
                    target_bytes: raw_os_bytes(raw_target.as_os_str()),
                },
                limits,
            )?;
        } else if metadata.is_dir() {
            let child = open_captured_directory(directory, &name, &display_path)?;
            push_entry(
                entries,
                logical_path.clone(),
                SourceEntryKind::Directory,
                limits,
            )?;
            visit_directory(
                root_directory,
                &child,
                &display_path,
                logical_path,
                depth + 1,
                root,
                limits,
                policy,
                captured_file_bytes,
                entries,
            )?;
        } else if metadata.is_file() {
            let remaining = limits.max_bytes.checked_sub(*captured_file_bytes).ok_or(
                SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                },
            )?;
            let (bytes, executable) = read_capability_file_bounded(
                directory,
                &name,
                &display_path,
                remaining,
                limits.max_bytes,
            )?;
            *captured_file_bytes = captured_file_bytes.checked_add(bytes.len() as u64).ok_or(
                SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                },
            )?;
            push_entry(
                entries,
                logical_path,
                SourceEntryKind::File { bytes, executable },
                limits,
            )?;
        } else {
            return Err(SourceResolveError::UnsupportedFileType { path: display_path });
        }
    }
    Ok(())
}

fn read_and_validate_symlink_target(
    root_directory: &CapabilityDirectory,
    root: &Path,
    directory: &CapabilityDirectory,
    logical_directory: &Path,
    name: &OsStr,
    link: &Path,
    policy: SourceTreePolicy,
) -> Result<PathBuf, SourceResolveError> {
    // Package-local policy hashes link spelling, requires an existing canonical target inside this
    // root, and rejects targets under paths excluded from that package view. Exact resolver-owned
    // materializations have no exclusions. Target contents are visited independently through the
    // ordinary tree walk rather than dereferenced through the link.
    let raw_target = directory
        .read_link_contents(name)
        .map_err(|error| io_error(link, error))?;
    if raw_target.is_absolute() {
        return Err(SourceResolveError::SymlinkEscapesRoot {
            link: link.to_path_buf(),
            target: raw_target,
        });
    }
    let target_request = logical_directory.join(&raw_target);
    let target_display = root.join(&target_request);
    let relative_target = root_directory.canonicalize(&target_request).map_err(|_| {
        SourceResolveError::SymlinkEscapesRoot {
            link: link.to_path_buf(),
            target: target_display,
        }
    })?;
    if policy == SourceTreePolicy::LocalPackage
        && relative_target
            .components()
            .any(|component| component.as_os_str() == ".git")
    {
        return Err(SourceResolveError::SymlinkTargetsExcludedMetadata {
            link: link.to_path_buf(),
            target: root.join(&relative_target),
        });
    }
    if policy == SourceTreePolicy::LocalPackage
        && relative_target
            .components()
            .next()
            .is_some_and(|component| component.as_os_str() == DEFAULT_BUILD_OUTPUT_DIRECTORY)
    {
        return Err(SourceResolveError::SymlinkTargetsExcludedBuildOutput {
            link: link.to_path_buf(),
            target: root.join(&relative_target),
        });
    }
    Ok(raw_target)
}

fn push_entry(
    entries: &mut Vec<SourceEntry>,
    relative: PathBuf,
    kind: SourceEntryKind,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    if entries.len() >= limits.max_files {
        return Err(SourceResolveError::TooManyFiles {
            limit: limits.max_files,
        });
    }
    entries.push(SourceEntry {
        relative_bytes: raw_os_bytes(relative.as_os_str()),
        relative_path: relative,
        kind,
    });
    Ok(())
}

pub(in crate::source) fn open_captured_directory(
    directory: &CapabilityDirectory,
    name: &OsStr,
    display_path: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let child = directory
        .open_dir_nofollow(name)
        .map_err(|error| io_error(display_path, error))?;
    let metadata = child
        .dir_metadata()
        .map_err(|error| io_error(display_path, error))?;
    if !metadata.is_dir() {
        return Err(SourceResolveError::UnsupportedFileType {
            path: display_path.to_path_buf(),
        });
    }
    Ok(child)
}

pub(in crate::source) fn read_capability_file_bounded(
    directory: &CapabilityDirectory,
    name: &OsStr,
    display_path: &Path,
    remaining: u64,
    limit: u64,
) -> Result<(Vec<u8>, bool), SourceResolveError> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = directory
        .open_with(name, &options)
        .map_err(|error| io_error(display_path, error))?;
    let metadata = file
        .metadata()
        .map_err(|error| io_error(display_path, error))?;
    if !metadata.is_file() {
        return Err(SourceResolveError::UnsupportedFileType {
            path: display_path.to_path_buf(),
        });
    }
    if metadata.len() > remaining {
        return Err(SourceResolveError::TooManyBytes { limit });
    }

    let initial_capacity =
        usize::try_from(metadata.len()).map_err(|_| SourceResolveError::TooManyBytes { limit })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(initial_capacity)
        .map_err(|_| SourceResolveError::TooManyBytes { limit })?;
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut chunk)
            .map_err(|error| io_error(display_path, error))?;
        if count == 0 {
            break;
        }
        let next_len = (bytes.len() as u64)
            .checked_add(count as u64)
            .ok_or(SourceResolveError::TooManyBytes { limit })?;
        if next_len > remaining {
            return Err(SourceResolveError::TooManyBytes { limit });
        }
        bytes.extend_from_slice(&chunk[..count]);
    }

    Ok((bytes, capability_metadata_is_executable(&metadata)))
}

#[cfg(unix)]
fn capability_metadata_is_executable(metadata: &cap_std::fs::Metadata) -> bool {
    use cap_fs_ext::OsMetadataExt;

    metadata.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn capability_metadata_is_executable(_metadata: &cap_std::fs::Metadata) -> bool {
    false
}

#[cfg(all(test, unix))]
pub(in crate::source) fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
#[allow(dead_code)] // Kept for the existing source-internal cross-platform facade.
pub(in crate::source) fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}

pub(in crate::source) fn raw_os_bytes(value: &OsStr) -> Vec<u8> {
    value.as_encoded_bytes().to_vec()
}

pub(in crate::source) struct SourceIdentityHasher {
    pub(in crate::source) hasher: Sha256,
    pub(in crate::source) byte_count: u64,
}

impl SourceIdentityHasher {
    pub(in crate::source) fn new(entry_count: usize) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"omega-source-tree-v4\0");
        hash_length(&mut hasher, entry_count as u64);
        Self {
            hasher,
            byte_count: 0,
        }
    }

    pub(in crate::source) fn add_directory(&mut self, relative_bytes: &[u8], normalized_mode: u16) {
        self.add_path(relative_bytes);
        self.hasher.update(b"directory");
        self.hasher.update(normalized_mode.to_le_bytes());
    }

    pub(in crate::source) fn add_file(
        &mut self,
        relative_bytes: &[u8],
        executable: bool,
        bytes: &[u8],
    ) -> Result<(), SourceResolveError> {
        self.add_path(relative_bytes);
        self.hasher.update(b"file");
        self.hasher.update([u8::from(executable)]);
        hash_bytes(&mut self.hasher, bytes);
        self.byte_count = self
            .byte_count
            .checked_add(bytes.len() as u64)
            .ok_or(SourceResolveError::TooManyBytes { limit: u64::MAX })?;
        Ok(())
    }

    pub(in crate::source) fn add_symlink(&mut self, relative_bytes: &[u8], target_bytes: &[u8]) {
        self.add_path(relative_bytes);
        self.hasher.update(b"symlink");
        hash_bytes(&mut self.hasher, target_bytes);
    }

    fn add_path(&mut self, relative_bytes: &[u8]) {
        self.hasher.update(b"entry");
        hash_bytes(&mut self.hasher, relative_bytes);
    }

    pub(in crate::source) fn finish(self) -> (u64, String) {
        (self.byte_count, format_sha256(&self.hasher.finalize()))
    }
}

pub(in crate::source) fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hash_length(hasher, bytes.len() as u64);
    hasher.update(bytes);
}

pub(in crate::source) fn hash_length(hasher: &mut Sha256, length: u64) {
    hasher.update(length.to_le_bytes());
}

pub(in crate::source) fn io_error(path: &Path, error: std::io::Error) -> SourceResolveError {
    SourceResolveError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}
