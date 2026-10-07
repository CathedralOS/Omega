//! REAL-filesystem provider for the interpreter (build.omg rungs 1+2,
//! TASKS_FS open-work #3): `build.omg` runs INTERPRETED with a real
//! `Filesystem` capability so it can copy assets itself. Strictly OPT-IN
//! (`FilesystemAccess::RealUnscoped` / `RealScoped`); the default hermetic
//! virtual fs -- the differential oracle -- is untouched.
//!
//! SCOPING (rung 2): under `RealScoped`, every path-taking op is authorized
//! against [`crate::FsGrants`] BEFORE the OS is touched -- reads must land
//! under a read or write root, writes/creates/removes under a write root;
//! refusal is -1/EACCES, the same shape as an OS permission denial, so the
//! wrapper's error surface needs no new cases. Paths and roots are
//! CANONICALIZED for the check. Operations that follow their final component
//! authorize the resolved target; operations that create, remove, or replace
//! a namespace leaf canonicalize only its parent and authorize the leaf
//! itself. Thus `..` traversal and parent symlink escapes are refused without
//! mistaking an existing leaf symlink's target for the entry a namespace
//! syscall actually mutates. Every fd retains whether its rooted origin had a
//! write grant; descriptor-based writes, metadata mutations, and host-visible
//! file locks re-check that bit before sponsor or host access. A read-authorized
//! source descriptor can therefore never amplify into mutation authority.
//!
//! Portable by construction: real files ride `std::fs::File` behind the same
//! synthetic-fd table shape the virtual fs uses (no libc, no raw handles), so
//! the provider works wherever the compiler runs. Both providers exhaustively
//! match the same closed operation set. FULL OP PARITY as of 2026-07-10m: every
//! op the virtual fs serves, the real provider serves too (unix-gated where
//! std requires it: symlink/permissions/chown; ENOTSUP on other hosts) --
//! so a build program tested hermetically cannot hit a refusal surprise in
//! real mode on the same host family.
//!
//! Every operation dispatches from `try_real_filesystem_call` to its
//! `real_*` method in `descriptor_calls`, `path_calls` or `directory_calls`.

mod descriptor_calls;
mod directory_calls;
mod grants;
mod host_io;
mod path_calls;
mod sponsor;

use crate::interpreter::evaluator::{
    EvalResult, FilesystemGrantAccess, FilesystemGrantRefusal, FilesystemGrantRefusalReason,
    PreparedByteOutput, PreparedFilesystemCall, Value,
};
use crate::{
    CanonicalFilesystemMetadataRowKind, FilesystemAuthorizedPath, FilesystemGrantRootIdentity,
    FilesystemMetadataObservationKind,
};
use grants::{Grants, canonical_grants};
use host_io::{open_options_for, open_real, real_os_bytes, real_path};

use std::collections::BTreeMap;

use std::path::{Path, PathBuf};

/// Errno for real-mode failures, from the host `io::Error` when available.
fn io_errno(error: &std::io::Error) -> i32 {
    error.raw_os_error().unwrap_or(5) // EIO when the host gives no code
}

/// ENOTSUP differs per OS (macOS 45, linux 95, windows maps EOPNOTSUPP=130);
/// the wrapper only tests `rc < 0` + errno passthrough, so macOS's value is
/// fine as the single modeled "this provider slice does not do that" code.
#[cfg(not(unix))]
const ENOTSUP: i32 = 45;
const EBADF: i32 = 9;
const EACCES: i32 = 13;
const EINVAL: i32 = 22;
const ENOENT: i32 = 2;
const ENOTDIR: i32 = 20;
const MAX_FILESYSTEM_OBSERVATION_PATH_BYTES: usize = 16 * 1024 * 1024;

/// One real open descriptor: the file handle plus the RESOLVED path it was
/// opened at (kept for the ops std serves path-wise, e.g. `read_dir` -- a
/// directory listing needs the path back, since std has no fd-based dirent
/// read).
struct RealFd {
    file: std::fs::File,
    path: PathBuf,
    sponsor_descriptor: Option<crate::FilesystemOpenDescriptor>,
    append: bool,
    /// Whether this descriptor's canonical rooted origin is covered by a
    /// compiler write grant. This is independent of the OS open mode: metadata
    /// operations can succeed on read-only descriptors on some hosts.
    write_granted: bool,
    /// Canonical immutable-source row selected at open time. `None` means the
    /// descriptor belongs to an ordinary root and retains physical metadata.
    canonical_metadata: Option<CanonicalFilesystemMetadataRowKind>,
}

pub(in crate::interpreter) struct RealFs {
    /// Synthetic fd -> real open file. Same table shape as `virtual_fds`;
    /// descriptors start at 3 (0/1/2 are the standard streams).
    files: BTreeMap<i32, RealFd>,
    next_fd: i32,
    /// Thread-local errno model, mirroring `virtual_errno`: set from the host
    /// `io::Error` on a failing op, read back by `errno`.
    pub(in crate::interpreter::evaluator) errno: i32,
    /// `Some` under `FilesystemAccess::RealScoped`; `None` is unscoped.
    grants: Option<Grants>,
    sponsor: Option<crate::FilesystemSponsor>,
}

impl RealFs {
    pub(in crate::interpreter) fn new(
        grants: Option<crate::FsGrants>,
        sponsor: Option<crate::FilesystemSponsor>,
    ) -> Result<Self, String> {
        let grants = grants.map(canonical_grants).transpose()?;
        Ok(Self {
            files: BTreeMap::new(),
            next_fd: 3,
            errno: 0,
            grants,
            sponsor,
        })
    }

    pub(in crate::interpreter::evaluator) fn is_scoped(&self) -> bool {
        self.grants.is_some()
    }

    pub(in crate::interpreter::evaluator) fn rooted_path_bytes(
        &self,
        identity: FilesystemGrantRootIdentity,
        relative: &[u8],
    ) -> Option<Vec<u8>> {
        let root = self.grants.as_ref()?.root(identity)?;
        let relative = real_path(relative)?;
        real_os_bytes(root.path.join(relative).as_os_str())
    }

    fn insert(
        &mut self,
        file: std::fs::File,
        path: PathBuf,
        sponsor_descriptor: Option<crate::FilesystemOpenDescriptor>,
        append: bool,
    ) -> Result<i64, i32> {
        let canonical_metadata = self.canonical_metadata_for_path(&path)?;
        if let Some(kind) = canonical_metadata {
            let metadata = file.metadata().map_err(|error| io_errno(&error))?;
            if !host_metadata_matches_canonical_kind(&metadata, kind) {
                return Err(EACCES);
            }
        }
        Ok(self.insert_preselected(file, path, sponsor_descriptor, append, canonical_metadata))
    }

    fn insert_preselected(
        &mut self,
        file: std::fs::File,
        path: PathBuf,
        sponsor_descriptor: Option<crate::FilesystemOpenDescriptor>,
        append: bool,
        canonical_metadata: Option<CanonicalFilesystemMetadataRowKind>,
    ) -> i64 {
        let write_granted = self
            .grants
            .as_ref()
            .is_none_or(|grants| grants.matching_root(&path, true).is_some());
        let fd = self.next_fd;
        self.next_fd += 1;
        self.files.insert(
            fd,
            RealFd {
                file,
                path,
                sponsor_descriptor,
                append,
                write_granted,
                canonical_metadata,
            },
        );
        i64::from(fd)
    }

    /// Select metadata for an already-authorized physical path. `Ok(None)` is
    /// the explicit physical-metadata policy; a canonical root with no exact
    /// row returns `EACCES` and must never fall back to the host value.
    fn canonical_metadata_for_path(
        &self,
        path: &Path,
    ) -> Result<Option<CanonicalFilesystemMetadataRowKind>, i32> {
        let Some(grants) = self.grants.as_ref() else {
            return Ok(None);
        };
        let Some(root) = grants.matching_root(path, false) else {
            return Err(EACCES);
        };
        let Some(index) = root.canonical_metadata.as_ref() else {
            return Ok(None);
        };
        let relative = path
            .strip_prefix(&root.path)
            .ok()
            .and_then(canonical_relative_path)
            .ok_or(EACCES)?;
        index.row(&relative).map(Some).ok_or(EACCES)
    }

    fn descriptor_write_granted(&self, fd: i32) -> Option<bool> {
        self.files.get(&fd).map(|entry| entry.write_granted)
    }
}

impl Drop for RealFs {
    fn drop(&mut self) {
        let Some(sponsor) = self.sponsor.clone() else {
            return;
        };
        // Evaluator return and every Halt path drop RealFs. Close accounting is
        // best-effort here because Drop cannot return a Halt; explicit close
        // operations still propagate every sponsor invariant failure.
        while let Some(fd) = self.files.keys().next().copied() {
            let descriptor = self
                .files
                .get(&fd)
                .and_then(|entry| entry.sponsor_descriptor);
            let prepared = descriptor
                .as_ref()
                .and_then(|descriptor| sponsor.prepare_close(descriptor).ok());
            self.files.remove(&fd);
            if let Some(prepared) = prepared {
                let _ = prepared.commit();
            }
        }
    }
}

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
fn canonical_relative_path(path: &Path) -> Option<Vec<u8>> {
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
    /// Mirror of `try_filesystem_call` against the REAL filesystem. The match
    /// exhaustively covers the same closed operation type as the virtual
    /// provider, so neither provider can silently omit a canonical operation.
    pub(in crate::interpreter::evaluator) fn try_real_filesystem_call(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<Value> {
        let result: i64 = match call {
            PreparedFilesystemCall::Create { .. } => self.real_create(call)?,
            PreparedFilesystemCall::Open { path, flags } => {
                self.real_open_prepared(path, flags, 0, false)?
            }
            PreparedFilesystemCall::OpenCreate { .. } => self.real_open_create(call)?,
            PreparedFilesystemCall::OpenPathHandle { .. } => self.real_open_path_handle(call)?,
            PreparedFilesystemCall::Read { .. } => self.real_read(call)?,
            PreparedFilesystemCall::Write { .. } => self.real_write(call)?,
            PreparedFilesystemCall::Seek { .. } => self.real_seek(call)?,
            PreparedFilesystemCall::Close { .. } => self.real_close(call)?,
            PreparedFilesystemCall::CloseHandle { .. } => self.real_close_handle(call)?,
            PreparedFilesystemCall::Duplicate { .. } => self.real_duplicate(call)?,
            PreparedFilesystemCall::SetLen { .. } => self.real_set_len(call)?,
            PreparedFilesystemCall::Sync { .. } | PreparedFilesystemCall::SyncData { .. } => {
                self.real_sync(call)?
            }
            // `remove_name` is the TRUSTED plain-path twin (D-at trust class):
            // the arg bytes ARE the path, so both spellings share one arm.
            PreparedFilesystemCall::Remove { .. } | PreparedFilesystemCall::RemoveName { .. } => {
                self.real_remove(call)?
            }
            PreparedFilesystemCall::CreateDir { path, mode: _ }
            | PreparedFilesystemCall::CreateDirName {
                name: path,
                mode: _,
            } => match self.authorized_namespace_leaf(&path, true, 0) {
                Some(path) => {
                    let prepared = self.prepare_sponsored_create_directory(&path)?;
                    let outcome = std::fs::create_dir(path);
                    self.finish_real_mutation(outcome, prepared, false)?
                }
                None => -1,
            },
            PreparedFilesystemCall::RemoveDir { .. }
            | PreparedFilesystemCall::RemoveDirName { .. } => self.real_remove_dir(call)?,
            PreparedFilesystemCall::Rename { .. } => self.real_rename(call)?,
            PreparedFilesystemCall::ReadAt { .. } => self.real_read_at(call)?,
            PreparedFilesystemCall::WriteAt { .. } => self.real_write_at(call)?,
            PreparedFilesystemCall::ReadDir { .. } => self.real_read_dir(call)?,
            PreparedFilesystemCall::FindFirst { .. } => self.real_find_first(call)?,
            PreparedFilesystemCall::FindNext { .. } => self.real_find_next(call)?,
            PreparedFilesystemCall::FindClose { .. } => self.real_find_close(call)?,
            PreparedFilesystemCall::ReadMetadata { path, buffer } => self.real_read_metadata(
                path,
                &buffer,
                FilesystemMetadataObservationKind::FollowedPath,
            )?,
            PreparedFilesystemCall::ReadSymlinkMetadata { path, buffer } => self
                .real_read_metadata(
                    path,
                    &buffer,
                    FilesystemMetadataObservationKind::UnfollowedFinalPath,
                )?,
            PreparedFilesystemCall::ReadFileMetadata { .. } => {
                self.real_read_file_metadata(call)?
            }
            PreparedFilesystemCall::Errno => i64::from(self.real_fs_mut().errno),
            PreparedFilesystemCall::Canonicalize { .. } => self.real_canonicalize(call)?,
            PreparedFilesystemCall::HardLink { .. } => self.real_hard_link(call)?,
            PreparedFilesystemCall::CreateHardLink { .. } => self.real_create_hard_link(call)?,
            PreparedFilesystemCall::GetOsfHandle { .. } => self.real_get_osf_handle(call)?,
            PreparedFilesystemCall::FinalPathNameByHandle { .. } => {
                self.real_final_path_name_by_handle(call)?
            }
            PreparedFilesystemCall::SetFileTime { .. } => self.real_set_file_time(call)?,
            PreparedFilesystemCall::Symlink { .. } => self.real_symlink(call)?,
            PreparedFilesystemCall::ReadLink { .. } => self.real_read_link(call)?,
            PreparedFilesystemCall::SetPermissions { .. } => self.real_set_permissions(call)?,
            PreparedFilesystemCall::SetFilePermissions { .. } => {
                self.real_set_file_permissions(call)?
            }
            PreparedFilesystemCall::SetFileTimes { .. } => self.real_set_file_times(call)?,
            PreparedFilesystemCall::LockFile { .. } => self.real_lock_file(call)?,
            PreparedFilesystemCall::LockFileEx { .. } => self.real_lock_file_ex(call)?,
            PreparedFilesystemCall::UnlockFile { .. } => self.real_unlock_file(call)?,
            PreparedFilesystemCall::GetLastError => i64::from(self.real_fs_mut().errno),
            PreparedFilesystemCall::ChangeOwner { .. } => self.real_change_owner(call)?,
            PreparedFilesystemCall::ChangeOwnerNoFollow { path, uid, gid } => {
                self.real_change_owner_no_follow(path, uid, gid)
            }
            PreparedFilesystemCall::ChangeFileOwner { .. } => self.real_change_file_owner(call)?,
            PreparedFilesystemCall::UnlinkAt { .. } => self.real_unlink_at(call)?,
            PreparedFilesystemCall::OpenAt { .. } => self.real_open_at(call)?,
        };
        Ok(Value::Int(result))
    }

    fn real_open_prepared(
        &mut self,
        path: Vec<u8>,
        flags: i32,
        mode: u32,
        create_variant: bool,
    ) -> EvalResult<i64> {
        let access = flags & 0x3;
        let wants_write = access == 1
            || access == 2
            || crate::interpreter::evaluator::filesystem::host_open_flags::o_creat(flags)
            || crate::interpreter::evaluator::filesystem::host_open_flags::o_trunc(flags)
            || crate::interpreter::evaluator::filesystem::host_open_flags::o_append(flags);
        match self.authorized_path(&path, wants_write, 0) {
            Some(path) => {
                let prepared = self.prepare_sponsored_open(
                    &path,
                    crate::interpreter::evaluator::filesystem::host_open_flags::o_creat(flags),
                    crate::interpreter::evaluator::filesystem::host_open_flags::o_trunc(flags),
                    wants_write,
                )?;
                let options = open_options_for(flags, mode, create_variant);
                let opened = open_real(&options, &path, wants_write);
                self.finish_real_open(
                    opened,
                    path,
                    crate::interpreter::evaluator::filesystem::host_open_flags::o_append(flags),
                    prepared,
                    false,
                )
            }
            None => Ok(-1),
        }
    }

    fn real_read_metadata(
        &mut self,
        path: Vec<u8>,
        buffer: &PreparedByteOutput,
        kind: FilesystemMetadataObservationKind,
    ) -> EvalResult<i64> {
        let no_follow = kind == FilesystemMetadataObservationKind::UnfollowedFinalPath;
        let authorized = if no_follow {
            self.authorized_path_no_follow(&path, false, 0)
        } else {
            self.authorized_path(&path, false, 0)
        };
        let Some(path) = authorized else {
            return Ok(-1);
        };
        let looked_up = if no_follow {
            std::fs::symlink_metadata(&path)
        } else {
            std::fs::metadata(&path)
        };
        match looked_up {
            Ok(metadata) => {
                let selected = self
                    .real_fs
                    .as_ref()
                    .expect("real metadata calls have a real provider")
                    // No-follow authorization already returned the
                    // parent-resolved authored leaf coordinate; followed
                    // authorization returned the canonical target.
                    .canonical_metadata_for_path(&path);
                match selected {
                    Ok(Some(canonical))
                        if host_metadata_matches_canonical_kind(&metadata, canonical) =>
                    {
                        self.write_canonical_fs_stat(buffer, kind, canonical)?;
                    }
                    Ok(Some(_)) | Err(_) => {
                        self.real_fs_mut().errno = EACCES;
                        return Ok(-1);
                    }
                    Ok(None) => self.write_real_fs_stat(buffer, kind, &metadata)?,
                }
                Ok(0)
            }
            Err(error) => {
                self.real_fs_mut().errno = io_errno(&error);
                Ok(-1)
            }
        }
    }

    fn real_change_owner_no_follow(&mut self, path: Vec<u8>, uid: i32, gid: i32) -> i64 {
        let Some(path) = self.authorized_path_no_follow(&path, true, 0) else {
            return -1;
        };
        #[cfg(unix)]
        {
            let owner = (uid >= 0).then_some(uid as u32);
            let group = (gid >= 0).then_some(gid as u32);
            self.real_result_unit(std::os::unix::fs::lchown(path, owner, group))
        }
        #[cfg(not(unix))]
        {
            let _ = (path, uid, gid);
            self.real_fs_mut().errno = ENOTSUP;
            -1
        }
    }

    fn real_fs_mut(&mut self) -> &mut RealFs {
        self.real_fs
            .as_mut()
            .expect("try_real_filesystem_call only runs in real mode")
    }

    /// Authorize a path-taking op against the grants (no-op when unscoped).
    /// `None` means REFUSED with errno already set: EACCES outside the
    /// granted roots, ENOENT when the path's parent does not even resolve.
    fn authorized_path(
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
    fn authorized_path_no_follow(
        &mut self,
        path_bytes: &[u8],
        write: bool,
        operand_ordinal: u8,
    ) -> Option<PathBuf> {
        self.authorized_path_with_follow(path_bytes, write, false, operand_ordinal)
    }

    fn authorized_namespace_leaf(
        &mut self,
        path_bytes: &[u8],
        write: bool,
        operand_ordinal: u8,
    ) -> Option<PathBuf> {
        self.authorized_path_no_follow(path_bytes, write, operand_ordinal)
    }

    fn authorized_path_with_follow(
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

    fn authorized_native_path(
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

    fn record_authorized_path(
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

    fn record_grant_refusal(
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

    #[cfg(unix)]
    fn real_result_unit(&mut self, outcome: std::io::Result<()>) -> i64 {
        match outcome {
            Ok(()) => 0,
            Err(error) => {
                self.real_fs_mut().errno = io_errno(&error);
                -1
            }
        }
    }

    /// Normalize the currently implemented real metadata semantics, then let
    /// the shared selected-target writer produce both carrier bytes and the
    /// canonical observation row. Native-field parity beyond mode, size, and
    /// mtime remains separate work; modeled fields are explicit rather than
    /// accidental host-layout residue.
    fn write_real_fs_stat(
        &mut self,
        output: &PreparedByteOutput,
        kind: FilesystemMetadataObservationKind,
        metadata: &std::fs::Metadata,
    ) -> EvalResult<()> {
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            let type_bits = if metadata.is_dir() {
                0o040000
            } else if metadata.file_type().is_symlink() {
                0o120000
            } else {
                0o100000
            };
            (type_bits | (metadata.permissions().mode() & 0o7777)) as u16
        };
        #[cfg(not(unix))]
        let mode: u16 = if metadata.is_dir() {
            0o040000
        } else {
            0o100000
        };
        let size = metadata.len() as i64;
        let mtime_secs = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        self.write_fs_stat(output, kind, u32::from(mode), size, mtime_secs)
    }

    fn write_canonical_fs_stat(
        &mut self,
        output: &PreparedByteOutput,
        kind: FilesystemMetadataObservationKind,
        metadata: CanonicalFilesystemMetadataRowKind,
    ) -> EvalResult<()> {
        let (mode, size, modification_time) = canonical_metadata_values(metadata);
        self.write_fs_stat(output, kind, mode, size, modification_time)
    }
}

enum SelectedFilesystemMetadata {
    Canonical(CanonicalFilesystemMetadataRowKind),
    Physical(std::fs::Metadata),
}

fn canonical_metadata_values(metadata: CanonicalFilesystemMetadataRowKind) -> (u32, i64, i64) {
    let (mode, size) = match metadata {
        CanonicalFilesystemMetadataRowKind::Directory => (0o040555, 0),
        CanonicalFilesystemMetadataRowKind::File {
            executable,
            logical_byte_length,
        } => (
            if executable { 0o100555 } else { 0o100444 },
            logical_byte_length as i64,
        ),
        CanonicalFilesystemMetadataRowKind::Symlink {
            target_spelling_logical_byte_length,
        } => (0o120777, target_spelling_logical_byte_length as i64),
    };
    (mode, size, 1_000_000_000)
}

fn host_metadata_matches_canonical_kind(
    metadata: &std::fs::Metadata,
    canonical: CanonicalFilesystemMetadataRowKind,
) -> bool {
    match canonical {
        CanonicalFilesystemMetadataRowKind::Directory => metadata.is_dir(),
        CanonicalFilesystemMetadataRowKind::File { .. } => metadata.is_file(),
        CanonicalFilesystemMetadataRowKind::Symlink { .. } => metadata.file_type().is_symlink(),
    }
}
