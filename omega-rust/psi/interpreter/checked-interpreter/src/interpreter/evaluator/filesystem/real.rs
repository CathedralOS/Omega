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

mod authorized_paths;
mod descriptor_calls;
mod directory_calls;
mod grants;
mod host_io;
mod metadata;
mod path_calls;
mod sponsor;

use crate::interpreter::evaluator::{EvalResult, PreparedFilesystemCall, Value};
use crate::{
    CanonicalFilesystemMetadataRowKind, FilesystemGrantRootIdentity,
    FilesystemMetadataObservationKind,
};
use authorized_paths::canonical_relative_path;
use grants::{Grants, canonical_grants};
use host_io::{open_options_for, open_real, real_os_bytes, real_path};
use metadata::host_metadata_matches_canonical_kind;

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

    fn real_fs_mut(&mut self) -> &mut RealFs {
        self.real_fs
            .as_mut()
            .expect("try_real_filesystem_call only runs in real mode")
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
}
