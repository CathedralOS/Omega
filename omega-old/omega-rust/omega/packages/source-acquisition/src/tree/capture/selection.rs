//! Select the capture namespace before opening file contents. Whole trees and
//! caller-selected inputs use the same reader and closing identity checks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cap_std::fs::Dir as CapabilityDirectory;
use filesystem_authority::{SourceCaptureRequest, SourceSelectionError, select_source_members};

use super::traversal::{
    CapturedDirectoryObservation, CapturedMemberObservation, CapturedTreeObservations,
    canonical_relative_path_bytes, close_captured_directory, excluded_from_capture,
    open_observed_directory, push_entry, read_and_validate_symlink_target,
};
use super::traversal_observations::{SourceEntry, SourceEntryKind, SourceTreePolicy};
use crate::SourceResolveError;
use crate::error::{too_many_bytes, too_many_files};
use crate::limits::{LocalSourceLimits, SOURCE_BYTE_ABSOLUTE_LIMIT};
use crate::tree::filesystem::{
    CapturedEntryObservation, IoResultAt, io_error, read_capability_file_bounded,
    require_unchanged_entry,
};

pub(super) fn capture_members(
    root_directory: &CapabilityDirectory,
    root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
    request: &SourceCaptureRequest,
    entries: &mut Vec<SourceEntry>,
    observations: &mut CapturedTreeObservations,
) -> Result<(), SourceResolveError> {
    let selected = select_source_members(
        request,
        (
            limits.max_entries.saturating_add(1),
            SOURCE_BYTE_ABSOLUTE_LIMIT as usize,
        ),
        |relative| {
            let path = source_path(relative, root)?;
            let mut prefix = PathBuf::new();
            for component in path.components() {
                if excluded_from_capture(policy, &prefix, component.as_os_str()) {
                    return Ok(None);
                }
                prefix.push(component.as_os_str());
            }
            if path.as_os_str().is_empty() {
                return root_directory.dir_metadata().at(root).map(Some);
            }
            let display = root.join(&path);
            let parent = path.parent().unwrap_or(Path::new(""));
            let directory = open_observed_directory(root_directory, parent, &root.join(parent))?;
            let name = path.file_name().expect("nonempty selected source path");
            match directory.symlink_metadata(name) {
                Ok(metadata) => {
                    if metadata.is_dir() && path.components().count() > limits.max_depth {
                        return Err(SourceResolveError::TooDeep {
                            path: display,
                            limit: limits.max_depth,
                        });
                    }
                    Ok(Some(metadata))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(io_error(&display, error)),
            }
        },
        cap_std::fs::Metadata::is_dir,
        |relative| {
            let path = source_path(relative, root)?;
            if path.components().count() > limits.max_depth {
                return Err(SourceResolveError::TooDeep {
                    path: root.join(path),
                    limit: limits.max_depth,
                });
            }
            let display = root.join(&path);
            let directory = open_observed_directory(root_directory, &path, &display)?;
            let opening = directory.dir_metadata().at(&display)?;
            let allowance = if policy == SourceTreePolicy::LocalPackage {
                if path.as_os_str().is_empty() { 4 } else { 1 }
            } else {
                0
            };
            let mut names = Vec::new();
            for member in directory.entries().at(&display)? {
                if names.len() >= limits.max_entries.saturating_add(allowance) {
                    return Err(too_many_files(limits.max_entries));
                }
                names.push(member.at(&display)?.file_name());
            }
            names.sort();
            let children = names
                .iter()
                .filter(|name| !excluded_from_capture(policy, &path, name))
                .map(|name| canonical_relative_path_bytes(&path.join(name)))
                .collect();
            observations.directories.push(CapturedDirectoryObservation {
                relative_path: path,
                metadata: opening,
                listed_names: Some(names),
            });
            Ok(children)
        },
        native_capture_path_is_canonical,
        |reason| match reason {
            SourceSelectionError::RowLimit { .. } => too_many_files(limits.max_entries),
            other => SourceResolveError::Io {
                path: root.to_path_buf(),
                message: other.to_string(),
            },
        },
    )?;

    // A partial ancestor was observed without enumerating its siblings. It
    // still participates in the final no-follow directory identity sweep.
    let mut enumerated = std::mem::take(&mut observations.directories)
        .into_iter()
        .map(|record| (canonical_relative_path_bytes(&record.relative_path), record))
        .collect::<BTreeMap<_, _>>();
    for (relative, metadata) in &selected {
        if metadata.is_dir() {
            let record = enumerated
                .remove(relative)
                .unwrap_or(CapturedDirectoryObservation {
                    relative_path: source_path(relative, root)?,
                    metadata: metadata.clone(),
                    listed_names: None,
                });
            observations.directories.push(record);
        }
    }

    let mut captured_file_bytes = 0u64;
    for (relative, opening) in &selected {
        if relative.is_empty() {
            continue;
        }
        let path = source_path(relative, root)?;
        let parent = path.parent().unwrap_or(Path::new(""));
        let display = root.join(&path);
        let directory = open_observed_directory(root_directory, parent, &root.join(parent))?;
        let name = path.file_name().expect("nonempty selected source path");
        require_unchanged_entry(
            opening,
            &directory.symlink_metadata(name).at(&display)?,
            &display,
        )?;
        observations.members.push(CapturedMemberObservation {
            relative_path: path.clone(),
            metadata: opening.clone(),
        });
        let kind = if opening.file_type().is_symlink() {
            let target = read_and_validate_symlink_target(
                root_directory,
                root,
                &directory,
                parent,
                name,
                &display,
                policy,
            )?;
            SourceEntryKind::Symlink {
                target_bytes: crate::tree::filesystem::raw_os_bytes(target.as_os_str()),
            }
        } else if opening.is_dir() {
            SourceEntryKind::Directory
        } else if opening.is_file() {
            let remaining = limits
                .max_bytes
                .checked_sub(captured_file_bytes)
                .ok_or(too_many_bytes(limits.max_bytes))?;
            let (bytes, executable) = read_capability_file_bounded(
                &directory,
                name,
                &display,
                remaining,
                limits.max_bytes,
            )?;
            captured_file_bytes = captured_file_bytes
                .checked_add(bytes.len() as u64)
                .ok_or(too_many_bytes(limits.max_bytes))?;
            SourceEntryKind::File { bytes, executable }
        } else {
            return Err(SourceResolveError::UnsupportedFileType { path: display });
        };
        push_entry(entries, path, kind, limits)?;
    }

    // Only selected subtrees were enumerated. Ancestors of explicitly named
    // files retain their metadata, without acquiring their unselected siblings.
    for record in observations.directories.iter().rev() {
        let Some(listed_names) = &record.listed_names else {
            continue;
        };
        let display = root.join(&record.relative_path);
        let directory = open_observed_directory(root_directory, &record.relative_path, &display)?;
        let mut members = Vec::new();
        for name in listed_names {
            let path = record.relative_path.join(name);
            if let Some(metadata) = selected.get(&canonical_relative_path_bytes(&path)) {
                members.push(CapturedEntryObservation {
                    name: name.clone(),
                    metadata: metadata.clone(),
                });
            }
        }
        close_captured_directory(
            &directory,
            &display,
            &record.metadata,
            listed_names,
            &members,
        )?;
    }
    Ok(())
}

fn native_capture_path_is_canonical(path: &[u8]) -> bool {
    #[cfg(unix)]
    {
        !path.is_empty()
            && !path.starts_with(b"/")
            && !path.contains(&0)
            && !path
                .split(|byte| *byte == b'/')
                .any(|component| component.is_empty() || matches!(component, b"." | b".."))
    }
    #[cfg(not(unix))]
    filesystem_authority::canonical_filesystem_metadata_path_is_canonical(path, false)
}

#[cfg(unix)]
fn source_path(bytes: &[u8], _root: &Path) -> Result<PathBuf, SourceResolveError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(std::ffi::OsString::from_vec(bytes.to_vec()).into())
}

#[cfg(not(unix))]
fn source_path(bytes: &[u8], root: &Path) -> Result<PathBuf, SourceResolveError> {
    String::from_utf8(bytes.to_vec())
        .map(PathBuf::from)
        .map_err(|error| {
            io_error(
                root,
                std::io::Error::new(std::io::ErrorKind::InvalidData, error),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::capture::traversal::verify_captured_tree;
    use crate::tree::filesystem::open_canonical_source_root;
    use filesystem_authority::SourceCaptureObligation;

    fn observed_selection(name: &str) -> (PathBuf, CapabilityDirectory, CapturedTreeObservations) {
        let root = crate::test_support::temp_root(name);
        std::fs::create_dir_all(root.join("private")).unwrap();
        std::fs::write(root.join("private/main.omg"), "selected source").unwrap();
        std::fs::write(root.join("private/unselected.txt"), "outside capture").unwrap();
        let directory = open_canonical_source_root(&root).unwrap();
        let request = SourceCaptureRequest::new([(
            b"private/main.omg".to_vec(),
            SourceCaptureObligation::Required,
        )])
        .unwrap();
        let mut observations = CapturedTreeObservations::default();
        capture_members(
            &directory,
            &root,
            LocalSourceLimits::default(),
            SourceTreePolicy::LocalPackage,
            &request,
            &mut Vec::new(),
            &mut observations,
        )
        .unwrap();
        (root, directory, observations)
    }

    #[test]
    fn closing_sweep_rejects_selected_file_drift() {
        let (root, directory, observations) = observed_selection("selected-file-drift");
        std::fs::write(
            root.join("private/main.omg"),
            "different selected source bytes",
        )
        .unwrap();
        assert!(matches!(
            verify_captured_tree(&directory, &root, &observations),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
        drop(directory);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn closing_sweep_preserves_capture_when_only_unselected_file_contents_change() {
        let (root, directory, observations) = observed_selection("unselected-file-drift");
        std::fs::write(
            root.join("private/unselected.txt"),
            "changed outside source inventory",
        )
        .unwrap();
        verify_captured_tree(&directory, &root, &observations).unwrap();
        drop(directory);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn closing_sweep_rejects_replaced_partial_ancestor_without_following_it() {
        let (root, directory, observations) = observed_selection("selected-ancestor-drift");
        std::fs::rename(root.join("private"), root.join("retained")).unwrap();
        std::os::unix::fs::symlink("retained", root.join("private")).unwrap();
        assert!(matches!(
            verify_captured_tree(&directory, &root, &observations),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
        drop(directory);
        std::fs::remove_dir_all(root).unwrap();
    }
}
