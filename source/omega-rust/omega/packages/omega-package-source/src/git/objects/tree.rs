//! Parsing and portable validation for recursive `git ls-tree` listings.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::error::SourceResolveError;
use crate::limits::{LocalSourceLimits, SOURCE_DEPTH_ABSOLUTE_LIMIT, SOURCE_ENTRY_ABSOLUTE_LIMIT};

use super::identity::is_object_id;
use super::{GitBlobBytes, GitTreeEntry, GitTreeEntryKind};

pub(crate) fn parse_git_tree_entries(
    listing: &[u8],
    repository: &Path,
    limits: LocalSourceLimits,
) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
    parse_git_tree_entries_with_policy(
        listing,
        repository,
        limits,
        GitTreePayloadLimitPolicy::WholeTree,
    )
}

/// Parse a complete recursive graph without charging unopened blob payloads to
/// the eventual package projection. The listing and graph remain bounded by
/// compiler-owned entry/depth ceilings and the process-output ceiling.
#[allow(dead_code)] // Used when the resolve layer adopts selective inspection.
pub(super) fn parse_git_tree_graph_entries(
    listing: &[u8],
    repository: &Path,
) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
    parse_git_tree_entries_with_policy(
        listing,
        repository,
        LocalSourceLimits {
            max_files: SOURCE_ENTRY_ABSOLUTE_LIMIT,
            max_bytes: u64::MAX,
            max_depth: SOURCE_DEPTH_ABSOLUTE_LIMIT,
        },
        GitTreePayloadLimitPolicy::SelectedOnly,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // `SelectedOnly` belongs to the staged selective-inspection API.
enum GitTreePayloadLimitPolicy {
    WholeTree,
    SelectedOnly,
}

fn parse_git_tree_entries_with_policy(
    listing: &[u8],
    repository: &Path,
    limits: LocalSourceLimits,
    payload_limit_policy: GitTreePayloadLimitPolicy,
) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
    let mut entries = Vec::new();
    let mut paths = BTreeMap::new();
    let mut blob_bytes = 0_u64;

    for record in listing.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
            return Err(git_tree_invalid(Vec::new(), "malformed ls-tree record"));
        };
        let header = &record[..tab];
        let path = &record[tab + 1..];
        let fields = header
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|field| !field.is_empty())
            .collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(git_tree_invalid(path, "malformed ls-tree header"));
        }
        let mode = fields[0];
        let object_type = fields[1];
        let oid = std::str::from_utf8(fields[2])
            .map_err(|_| git_tree_invalid(path, "object ID is not ASCII"))?;
        if !is_object_id(oid) {
            return Err(git_tree_invalid(path, "object ID has an invalid spelling"));
        }
        if mode == b"160000" || object_type == b"commit" {
            return Err(SourceResolveError::GitSubmodulesUnsupported {
                path: git_path_from_bytes(path).unwrap_or_else(|_| repository.to_path_buf()),
            });
        }
        let relative_path = validate_git_path(path, limits)?;
        if path
            .split(|byte| *byte == b'/')
            .any(|component| component.eq_ignore_ascii_case(b".gitmodules"))
        {
            return Err(SourceResolveError::GitSubmodulesUnsupported {
                path: relative_path,
            });
        }
        let (size, kind) = match (mode, object_type, fields[3]) {
            (b"040000", b"tree", b"-") => (0, GitTreeEntryKind::Tree),
            (b"100644", b"blob", size) => (
                parse_git_blob_size(path, size)?,
                GitTreeEntryKind::File {
                    executable: false,
                    bytes: GitBlobBytes::empty(),
                },
            ),
            (b"100755", b"blob", size) => (
                parse_git_blob_size(path, size)?,
                GitTreeEntryKind::File {
                    executable: true,
                    bytes: GitBlobBytes::empty(),
                },
            ),
            (b"120000", b"blob", size) => (
                parse_git_blob_size(path, size)?,
                GitTreeEntryKind::Symlink {
                    target_bytes: GitBlobBytes::empty(),
                },
            ),
            _ => return Err(git_tree_invalid(path, "unsupported Git tree entry")),
        };
        if paths
            .insert(path.to_vec(), matches!(&kind, GitTreeEntryKind::Tree))
            .is_some()
        {
            return Err(git_tree_invalid(path, "duplicate path"));
        }
        let identity_entry_count =
            entries
                .len()
                .checked_add(1)
                .ok_or(SourceResolveError::TooManyFiles {
                    limit: limits.max_files,
                })?;
        if identity_entry_count > limits.max_files {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_files,
            });
        }
        if payload_limit_policy == GitTreePayloadLimitPolicy::WholeTree
            && !matches!(&kind, GitTreeEntryKind::Tree)
        {
            blob_bytes = blob_bytes
                .checked_add(size)
                .ok_or(SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                })?;
            if blob_bytes > limits.max_bytes {
                return Err(SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                });
            }
        }
        entries.push(GitTreeEntry {
            relative_bytes: path.to_vec(),
            relative_path,
            oid: oid.to_owned(),
            size,
            kind,
        });
    }

    entries.sort_by(|left, right| left.relative_bytes.cmp(&right.relative_bytes));
    for entry in &entries {
        for separator in entry
            .relative_bytes
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'/').then_some(index))
        {
            let parent = &entry.relative_bytes[..separator];
            match paths.get(parent) {
                Some(true) => {}
                Some(false) => {
                    return Err(git_tree_invalid(
                        &entry.relative_bytes,
                        "Git path traverses a blob",
                    ));
                }
                None => {
                    return Err(git_tree_invalid(
                        &entry.relative_bytes,
                        "Git listing omitted a parent-tree edge",
                    ));
                }
            }
        }
    }
    Ok(entries)
}

fn parse_git_blob_size(path: &[u8], size: &[u8]) -> Result<u64, SourceResolveError> {
    std::str::from_utf8(size)
        .ok()
        .and_then(|size| size.parse::<u64>().ok())
        .ok_or_else(|| git_tree_invalid(path, "blob size is missing or invalid"))
}

pub(crate) fn git_directory_paths(entries: &[GitTreeEntry]) -> BTreeSet<Vec<u8>> {
    entries
        .iter()
        .filter(|entry| matches!(&entry.kind, GitTreeEntryKind::Tree))
        .map(|entry| entry.relative_bytes.clone())
        .collect()
}

pub(super) fn validate_git_path(
    path: &[u8],
    limits: LocalSourceLimits,
) -> Result<PathBuf, SourceResolveError> {
    if path.is_empty() || path.starts_with(b"/") || path.ends_with(b"/") {
        return Err(git_tree_invalid(
            path,
            "path must be a non-empty relative path",
        ));
    }
    if path.contains(&b'\\') {
        return Err(git_tree_invalid(
            path,
            "backslashes are forbidden in portable package paths",
        ));
    }
    let components = path.split(|byte| *byte == b'/').collect::<Vec<_>>();
    for component in &components {
        if component.is_empty() || *component == b"." || *component == b".." {
            return Err(git_tree_invalid(
                path,
                "path contains a traversal component",
            ));
        }
        if component.eq_ignore_ascii_case(b".git") {
            return Err(git_tree_invalid(path, "path enters excluded Git metadata"));
        }
        validate_portable_git_component(path, component)?;
    }
    let depth = components.len().saturating_sub(1);
    if depth > limits.max_depth {
        return Err(SourceResolveError::TooDeep {
            path: git_path_from_bytes(path)?,
            limit: limits.max_depth,
        });
    }
    git_path_from_bytes(path)
}

fn validate_portable_git_component(
    path: &[u8],
    component: &[u8],
) -> Result<(), SourceResolveError> {
    if component
        .iter()
        .any(|byte| *byte < 32 || matches!(*byte, b'<' | b'>' | b':' | b'"' | b'|' | b'?' | b'*'))
    {
        return Err(git_tree_invalid(
            path,
            "path contains a character forbidden by the portable Windows policy",
        ));
    }
    if component
        .last()
        .is_some_and(|byte| matches!(byte, b'.' | b' '))
    {
        return Err(git_tree_invalid(
            path,
            "path component has a Windows-ambiguous trailing dot or space",
        ));
    }
    let stem = component
        .split(|byte| *byte == b'.')
        .next()
        .unwrap_or(component);
    let reserved_device = [b"CON".as_slice(), b"PRN", b"AUX", b"NUL"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
        || (stem.len() == 4
            && (stem[..3].eq_ignore_ascii_case(b"COM") || stem[..3].eq_ignore_ascii_case(b"LPT"))
            && matches!(stem[3], b'1'..=b'9'))
        || stem.eq_ignore_ascii_case(b"CONIN$")
        || stem.eq_ignore_ascii_case(b"CONOUT$");
    if reserved_device {
        return Err(git_tree_invalid(
            path,
            "path component uses a reserved Windows device name",
        ));
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn git_path_from_bytes(path: &[u8]) -> Result<PathBuf, SourceResolveError> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    Ok(PathBuf::from(OsString::from_vec(path.to_vec())))
}

#[cfg(not(unix))]
pub(super) fn git_path_from_bytes(path: &[u8]) -> Result<PathBuf, SourceResolveError> {
    let text = std::str::from_utf8(path)
        .map_err(|_| git_tree_invalid(path, "path cannot be represented on this host"))?;
    Ok(PathBuf::from(text))
}

pub(crate) fn validate_git_symlink_target(
    link: &[u8],
    target: &[u8],
) -> Result<(), SourceResolveError> {
    if target.is_empty() || target.starts_with(b"/") || target.contains(&0) {
        return Err(git_tree_invalid(
            link,
            "symlink target must be a non-empty relative path",
        ));
    }
    if target.contains(&b'\\') {
        return Err(git_tree_invalid(
            link,
            "symlink target contains a non-portable path separator",
        ));
    }
    let mut depth = link.split(|byte| *byte == b'/').count().saturating_sub(1);
    for component in target.split(|byte| *byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| git_tree_invalid(link, "symlink target escapes the snapshot"))?;
            }
            component if component.eq_ignore_ascii_case(b".git") => {
                return Err(git_tree_invalid(
                    link,
                    "symlink target enters excluded Git metadata",
                ));
            }
            component => {
                validate_portable_git_component(link, component)?;
                depth += 1;
            }
        }
    }
    Ok(())
}

pub(crate) fn git_tree_invalid(
    path: impl AsRef<[u8]>,
    message: impl Into<String>,
) -> SourceResolveError {
    SourceResolveError::GitTreeInvalid {
        path: path.as_ref().to_vec(),
        message: message.into(),
    }
}
