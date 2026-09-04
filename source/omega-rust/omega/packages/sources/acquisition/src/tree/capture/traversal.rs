//! No-follow traversal and policy validation for source trees.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use cap_std::fs::Dir as CapabilityDirectory;

use super::model::{SourceEntry, SourceEntryKind, SourceTreePolicy};
use crate::SourceResolveError;
use crate::limits::{DEFAULT_BUILD_OUTPUT_DIRECTORY, LocalSourceLimits};
use crate::tree::filesystem::{
    io_error, open_captured_directory, raw_os_bytes, read_capability_file_bounded,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn visit_directory(
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

    let remaining_entries = limits.max_entries.saturating_sub(entries.len());
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
                limit: limits.max_entries,
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
        if entries.len() >= limits.max_entries {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_entries,
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
    if entries.len() >= limits.max_entries {
        return Err(SourceResolveError::TooManyFiles {
            limit: limits.max_entries,
        });
    }
    entries.push(SourceEntry {
        relative_bytes: canonical_relative_path_bytes(&relative),
        relative_path: relative,
        kind,
    });
    Ok(())
}

fn canonical_relative_path_bytes(relative: &Path) -> Vec<u8> {
    let mut encoded = Vec::new();
    for component in relative.components() {
        if !encoded.is_empty() {
            encoded.push(b'/');
        }
        encoded.extend_from_slice(raw_os_bytes(component.as_os_str()).as_slice());
    }
    encoded
}
