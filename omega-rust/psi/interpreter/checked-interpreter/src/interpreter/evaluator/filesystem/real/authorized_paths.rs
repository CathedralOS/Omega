use super::host_io::real_path;
use super::{EACCES, ENOENT, MAX_FILESYSTEM_OBSERVATION_PATH_BYTES};
use crate::interpreter::evaluator::{
    FilesystemGrantAccess, FilesystemGrantRefusal, FilesystemGrantRefusalReason,
};
use crate::{FilesystemAuthorizedPath, FilesystemGrantRootIdentity};
use std::path::{Path, PathBuf};

/// Parent-canonical resolution: ALWAYS canonicalize the parent and re-attach
/// the leaf, never the full path -- the no-follow variant's resolver (the
/// leaf may be a symlink the op must inspect, not traverse).
fn resolve_parent_for_check(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let name = path.file_name()?;
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".").canonicalize().ok()?
    } else {
        parent.canonicalize().ok()?
    };
    Some(parent.join(name))
}

/// Resolve a path to its REAL location for the grant check: canonicalize the
/// path itself when it exists, else walk the leaf's symlink chain by hand and
/// canonicalize the final target's parent, re-attaching that target's leaf
/// (the create/new-file case). `None` when even that parent does not resolve
/// -- the caller reports ENOENT, exactly what the OS would say.
///
/// The chain is walked rather than trusted because `canonicalize` fails on a
/// DANGLING leaf link, and re-attaching the original leaf then names the link
/// instead of the file the host would create. A scoped `build.omg` could
/// authorize `out/x -> /outside/new` (only the link name is authorized, and
/// its target need not exist), pass the grant check because `out` is the write
/// root, and have `open(O_CREAT)` follow the link and create the file outside
/// every grant. Reporting the location the open will actually reach makes the
/// check and the open agree: the escape is refused for being outside the root,
/// while a dangling link whose target is inside the root still opens, which
/// refusing every symlink leaf would have broken.
fn resolve_for_check(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = path.canonicalize() {
        return Some(canonical);
    }
    // One past Linux's MAXSYMLINKS, so a cycle among dangling links ends here
    // as an unresolvable path rather than running until the host complains.
    const LINK_LIMIT: usize = 41;
    let mut current = path.to_path_buf();
    for _ in 0..LINK_LIMIT {
        let Ok(target) = std::fs::read_link(&current) else {
            // Not a link: `current` is the real leaf the open will create.
            let parent = current.parent()?;
            let name = current.file_name()?;
            let parent = if parent.as_os_str().is_empty() {
                Path::new(".").canonicalize().ok()?
            } else {
                parent.canonicalize().ok()?
            };
            return Some(parent.join(name));
        };
        current = if target.is_absolute() {
            target
        } else {
            current.parent()?.join(target)
        };
        if let Ok(canonical) = current.canonicalize() {
            return Some(canonical);
        }
    }
    None
}

#[cfg(all(test, unix))]
mod confinement_tests;

/// Encode a canonical path beneath a canonical grant root without retaining
/// host separators or an absolute compiler path. Scoped execution rejects an
/// unrepresentable component before host access instead of applying a lossy
/// conversion to observation evidence.
pub(super) fn canonical_relative_path(path: &Path) -> Option<Vec<u8>> {
    let mut encoded = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(component) = component else {
            return None;
        };
        let component = component.to_str()?.as_bytes();
        if !encoded.is_empty() {
            encoded.push(b'/');
        }
        encoded.extend_from_slice(component);
    }
    Some(encoded)
}

impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
    /// Authorize a path-taking op against the grants (no-op when unscoped).
    /// `None` means REFUSED with errno already set: EACCES outside the
    /// granted roots, ENOENT when the path's parent does not even resolve.
    pub(super) fn authorized_path(
        &mut self,
        path_bytes: &[u8],
        write: bool,
        operand_ordinal: u8,
    ) -> Option<PathBuf> {
        self.authorized_path_with_follow(path_bytes, write, true, operand_ordinal)
    }

    /// The NO-FOLLOW variant for operations whose operand is the namespace
    /// leaf itself. This includes symlink-inspecting operations (read_link,
    /// read_symlink_metadata, lchown) and namespace mutations (mkdir, unlink,
    /// rmdir, rename, link, symlink). Full canonicalization would authorize
    /// and return an existing leaf symlink's TARGET even though the host call
    /// inspects, creates, removes, or replaces the symlink NAME. Resolution
    /// therefore canonicalizes the parent and reattaches the leaf.
    pub(super) fn authorized_path_no_follow(
        &mut self,
        path_bytes: &[u8],
        write: bool,
        operand_ordinal: u8,
    ) -> Option<PathBuf> {
        self.authorized_path_with_follow(path_bytes, write, false, operand_ordinal)
    }

    pub(super) fn authorized_namespace_leaf(
        &mut self,
        path_bytes: &[u8],
        write: bool,
        operand_ordinal: u8,
    ) -> Option<PathBuf> {
        self.authorized_path_no_follow(path_bytes, write, operand_ordinal)
    }

    pub(super) fn authorized_path_with_follow(
        &mut self,
        path_bytes: &[u8],
        write: bool,
        follow: bool,
        operand_ordinal: u8,
    ) -> Option<PathBuf> {
        let Some(path) = real_path(path_bytes) else {
            self.real_fs_mut().errno = EACCES;
            self.record_grant_refusal(
                operand_ordinal,
                write,
                FilesystemGrantRefusalReason::UnrepresentableRootedPath,
            );
            return None;
        };
        self.authorized_native_path(&path, write, follow, operand_ordinal)
    }

    pub(super) fn authorized_native_path(
        &mut self,
        path: &Path,
        write: bool,
        follow: bool,
        operand_ordinal: u8,
    ) -> Option<PathBuf> {
        let Some(grants) = self
            .real_fs
            .as_ref()
            .and_then(|filesystem| filesystem.grants.as_ref())
        else {
            return Some(path.to_path_buf()); // unscoped: full process authority
        };
        let resolved = if follow {
            resolve_for_check(path)
        } else {
            resolve_parent_for_check(path)
        };
        let Some(resolved) = resolved else {
            self.real_fs_mut().errno = ENOENT;
            self.record_grant_refusal(
                operand_ordinal,
                write,
                FilesystemGrantRefusalReason::Unresolvable,
            );
            return None;
        };
        if let Some(root) = grants.matching_root(&resolved, write) {
            let relative_path = resolved
                .strip_prefix(&root.path)
                .ok()
                .and_then(canonical_relative_path);
            let Some(relative_path) = relative_path else {
                self.real_fs_mut().errno = EACCES;
                self.record_grant_refusal(
                    operand_ordinal,
                    write,
                    FilesystemGrantRefusalReason::UnrepresentableRootedPath,
                );
                return None;
            };
            let root = root.identity;
            if !self.record_authorized_path(operand_ordinal, write, root, relative_path) {
                self.real_fs_mut().errno = EACCES;
                return None;
            }
            // Operate on the RESOLVED path: the authorized location and the
            // operated-on location must be the same real file.
            Some(resolved)
        } else {
            self.real_fs_mut().errno = EACCES;
            self.record_grant_refusal(
                operand_ordinal,
                write,
                FilesystemGrantRefusalReason::OutsideGrantedRoots,
            );
            None
        }
    }

    pub(super) fn record_authorized_path(
        &mut self,
        operand_ordinal: u8,
        write: bool,
        root: FilesystemGrantRootIdentity,
        relative_path: Vec<u8>,
    ) -> bool {
        let Some(next_total) = self
            .filesystem_observation_path_bytes
            .checked_add(relative_path.len())
            .filter(|total| *total <= MAX_FILESYSTEM_OBSERVATION_PATH_BYTES)
        else {
            self.record_grant_refusal(
                operand_ordinal,
                write,
                FilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded,
            );
            self.filesystem_observation_resource_halt = Some(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_PATH_BYTES}-byte rooted-path ceiling"
            ));
            return false;
        };
        let Some(attempt_index) = self.filesystem_operation_attempt_stack.last().copied() else {
            return false;
        };
        self.filesystem_observation_path_bytes = next_total;
        self.filesystem_operation_attempts[attempt_index]
            .authorized_paths
            .push(FilesystemAuthorizedPath::new(
                operand_ordinal,
                if write {
                    FilesystemGrantAccess::Write
                } else {
                    FilesystemGrantAccess::Read
                },
                root,
                relative_path,
            ));
        true
    }

    pub(super) fn record_grant_refusal(
        &mut self,
        operand_ordinal: u8,
        write: bool,
        reason: FilesystemGrantRefusalReason,
    ) {
        let Some(attempt_index) = self.filesystem_operation_attempt_stack.last().copied() else {
            return;
        };
        self.filesystem_operation_attempts[attempt_index]
            .grant_refusals
            .push(FilesystemGrantRefusal::new(
                operand_ordinal,
                if write {
                    FilesystemGrantAccess::Write
                } else {
                    FilesystemGrantAccess::Read
                },
                reason,
            ));
    }
}
