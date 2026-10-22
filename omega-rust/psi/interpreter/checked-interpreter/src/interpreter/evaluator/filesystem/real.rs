//! Filesystem execution over granted host roots and retained Build inputs.
//!
//! `RealUnscoped` and `RealScoped` select the physical host protocol.
//! `CapturedScoped` also binds immutable logical Source/input roots: those
//! dispatch through `captured`, have no host spelling or fallback, and expose
//! only their captured vocabulary, not fabricated native metadata or handles.
//! Its physical output root uses the same private append-and-seal sponsor.
//! The virtual filesystem remains a separate differential interpreter provider.
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
//! the provider works wherever the compiler runs. Physical and virtual
//! providers match the same operation set, with host-specific operations
//! returning ENOTSUP where unavailable. Captured roots deliberately offer a
//! smaller, target-neutral protocol; native parity is not their contract.
//!
//! Every operation dispatches from `try_real_filesystem_call` to its
//! `real_*` method in `descriptor_calls`, `path_calls` or `directory_calls`.

mod authorized_paths;
mod captured;
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
use host_io::{real_os_bytes, real_path};
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
    captured_roots: BTreeMap<FilesystemGrantRootIdentity, source::SourceSnapshot>,
    captured_descriptors: BTreeMap<i32, captured::CapturedDescriptor>,
    captured_descriptions: BTreeMap<i32, captured::CapturedOpenDescription>,
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
            captured_roots: BTreeMap::new(),
            captured_descriptors: BTreeMap::new(),
            captured_descriptions: BTreeMap::new(),
            errno: 0,
            grants,
            sponsor,
        })
    }

    pub(in crate::interpreter) fn with_captured_inputs(
        grants: crate::FsGrants,
        sponsor: Option<crate::FilesystemSponsor>,
        inputs: Vec<crate::CapturedFilesystemRoot>,
    ) -> Result<Self, String> {
        let mut filesystem = Self::new(Some(grants), sponsor)?;
        for input in inputs {
            if filesystem
                .grants
                .as_ref()
                .is_some_and(|grants| grants.root(input.identity).is_some())
                || filesystem
                    .captured_roots
                    .insert(input.identity, input.snapshot)
                    .is_some()
            {
                return Err(format!(
                    "filesystem root identity {} is duplicated across provider storage",
                    input.identity.get()
                ));
            }
        }
        Ok(filesystem)
    }

    /// Physical and captured descriptors share this checked allocator. No
    /// provider may recycle a number that observation custody has retired.
    fn allocate_descriptor(&mut self) -> Result<i32, i32> {
        let descriptor = self.next_fd;
        self.next_fd = descriptor.checked_add(1).ok_or(24)?;
        Ok(descriptor)
    }

    pub(in crate::interpreter::evaluator) fn descriptor_root(
        &self,
        descriptor: i32,
    ) -> Option<FilesystemGrantRootIdentity> {
        self.captured_descriptors
            .get(&descriptor)
            .map(|entry| entry.root)
            .or_else(|| {
                self.files.get(&descriptor).and_then(|entry| {
                    self.grants
                        .as_ref()?
                        .matching_root(&entry.path, false)
                        .map(|root| root.identity)
                })
            })
    }

    pub(in crate::interpreter::evaluator) fn prepare_rooted_path(
        &self,
        identity: FilesystemGrantRootIdentity,
        relative: &[u8],
    ) -> Option<super::preparation::PreparedFilesystemPath> {
        if self.captured_roots.contains_key(&identity) {
            return Some(super::preparation::PreparedFilesystemPath::Captured {
                root: identity,
                relative: relative.to_vec(),
            });
        }
        self.rooted_path_bytes(identity, relative)
            .map(super::preparation::PreparedFilesystemPath::Native)
    }

    /// Generated-source handoff consumes a sealed regular staging file, not
    /// merely an existing pathname. Enforce this at the untrusted operation;
    /// output capture does not replay the invocation's descriptor namespace.
    pub(in crate::interpreter::evaluator) fn require_sealed_rooted_file(
        &self,
        root: FilesystemGrantRootIdentity,
        relative: &[u8],
    ) -> EvalResult<()> {
        use crate::interpreter::evaluator::halts::filesystem_sponsor_halt;
        let sponsor = self.sponsor.as_ref().ok_or_else(|| {
            super::super::Halt::Trap("generated source requires sponsored output custody".into())
        })?;
        let path = self
            .rooted_path_bytes(root, relative)
            .and_then(|bytes| real_path(&bytes))
            .ok_or_else(|| {
                super::super::Halt::Trap("generated source names no physical output root".into())
            })?;
        let bound = match sponsor.bind_path(&path) {
            Ok(bound) => bound,
            Err(error) => return filesystem_sponsor_halt(error),
        };
        if let Err(error) = sponsor.require_sealed(&bound) {
            return filesystem_sponsor_halt(error);
        }
        match sponsor.entry(&bound) {
            Ok(Some(crate::FilesystemSponsorEntry::Object { .. })) => Ok(()),
            Ok(_) => Err(super::super::Halt::Trap(
                "generated source must name a sealed regular file".into(),
            )),
            Err(error) => filesystem_sponsor_halt(error),
        }
    }

    pub(in crate::interpreter::evaluator) fn call_uses_captured_input(
        &self,
        call: &PreparedFilesystemCall,
        plan: &super::preparation::PreparedFilesystemLogicalHandlePlan,
    ) -> bool {
        call.has_captured_path()
            || plan.inputs.iter().any(|input| {
                i32::try_from(input.raw)
                    .ok()
                    .is_some_and(|descriptor| self.captured_descriptors.contains_key(&descriptor))
            })
    }

    pub(in crate::interpreter::evaluator) fn is_scoped(&self) -> bool {
        self.grants.is_some()
    }

    fn rooted_path_bytes(
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
        self.insert_preselected(file, path, sponsor_descriptor, append, canonical_metadata)
    }

    fn insert_preselected(
        &mut self,
        file: std::fs::File,
        path: PathBuf,
        sponsor_descriptor: Option<crate::FilesystemOpenDescriptor>,
        append: bool,
        canonical_metadata: Option<CanonicalFilesystemMetadataRowKind>,
    ) -> Result<i64, i32> {
        let write_granted = self
            .grants
            .as_ref()
            .is_none_or(|grants| grants.matching_root(&path, true).is_some());
        let fd = self.allocate_descriptor()?;
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
        Ok(i64::from(fd))
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
    /// Mirror of `serve_filesystem_call` against the REAL filesystem. The match
    /// exhaustively covers the same closed operation type as the virtual
    /// provider, so neither provider can silently omit a canonical operation.
    pub(in crate::interpreter::evaluator) fn try_real_filesystem_call(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<Value> {
        let result: i64 = crate::prepared_filesystem_call_dispatch!(
                    self, call;
                    literal {
        PreparedFilesystemCall::Open { path, flags } => {
                        self.real_open_prepared(path.into_native()?, flags, 0, false)?
                    }
        PreparedFilesystemCall::CreateDir { path, mode: _ }
                    | PreparedFilesystemCall::CreateDirName {
                        name: path,
                        mode: _,
                    } => {
                        match self.authorized_namespace_leaf(&path, true, 0) {
                        Some(path) => {
                            if let Some(sponsor) = self.real_fs_mut().sponsor.clone() {
                                let bound = sponsor::sponsor_value(sponsor.bind_path(&path))?;
                                if matches!(
                                    sponsor::sponsor_value(sponsor.entry(&bound))?,
                                    Some(crate::FilesystemSponsorEntry::Directory)
                                ) {
                                    return Ok(Value::Int(0));
                                }
                            }
                            let prepared = self.prepare_sponsored_create_directory(&path)?;
                            let outcome = std::fs::create_dir(path);
                            self.finish_real_mutation(outcome, prepared, false)?
                        }
                        None => -1,
                        }
                    },
        PreparedFilesystemCall::ReadMetadata { path, buffer } => {
                        self.real_read_metadata(
                        path.into_native()?,
                        &buffer,
                        FilesystemMetadataObservationKind::FollowedPath,
                    )?
                    }
                    PreparedFilesystemCall::ReadSymlinkMetadata { path, buffer } => {
                        self.real_read_metadata(
                            path.into_native()?,
                            &buffer,
                            FilesystemMetadataObservationKind::UnfollowedFinalPath,
                        )?
                    }
                    PreparedFilesystemCall::Errno => { i64::from(self.real_fs_mut().errno) }
                    PreparedFilesystemCall::GetLastError => { i64::from(self.real_fs_mut().errno) }
                    PreparedFilesystemCall::ChangeOwnerNoFollow { path, uid, gid } => {
                        self.real_change_owner_no_follow(path.into_native()?, uid, gid)
                    }
                    }
                    dispatch {
                        Create => real_create,
                        OpenCreate => real_open_create,
                        OpenPathHandle => real_open_path_handle,
                        Read => real_read,
                        Write => real_write,
                        Seek => real_seek,
                        Close => real_close,
                        CloseHandle => real_close_handle,
                        Duplicate => real_duplicate,
                        SetLen => real_set_len,
                        Sync | SyncData => real_sync,
                        // `remove_name` is the TRUSTED plain-path twin (D-at trust
                        // class): the arg bytes ARE the path, so both spellings
                        // share one arm.
                        Remove | RemoveName => real_remove,
                        RemoveDir | RemoveDirName => real_remove_dir,
                        Rename => real_rename,
                        ReadAt => real_read_at,
                        WriteAt => real_write_at,
                        ReadDir => real_read_dir,
                        FindFirst => real_find_first,
                        FindNext => real_find_next,
                        FindClose => real_find_close,
                        ReadFileMetadata => real_read_file_metadata,
                        Canonicalize => real_canonicalize,
                        HardLink => real_hard_link,
                        CreateHardLink => real_create_hard_link,
                        GetOsfHandle => real_get_osf_handle,
                        FinalPathNameByHandle => real_final_path_name_by_handle,
                        SetFileTime => real_set_file_time,
                        Symlink => real_symlink,
                        ReadLink => real_read_link,
                        SetPermissions => real_set_permissions,
                        SetFilePermissions => real_set_file_permissions,
                        SetFileTimes => real_set_file_times,
                        LockFile => real_lock_file,
                        LockFileEx => real_lock_file_ex,
                        UnlockFile => real_unlock_file,
                        ChangeOwner => real_change_owner,
                        ChangeFileOwner => real_change_file_owner,
                        UnlinkAt => real_unlink_at,
                        OpenAt => real_open_at
                    }
                );
        Ok(Value::Int(result))
    }

    fn real_open_prepared(
        &mut self,
        path: Vec<u8>,
        flags: i32,
        mode: u32,
        create_variant: bool,
    ) -> EvalResult<i64> {
        let wants_write =
            crate::interpreter::evaluator::filesystem::host_open_flags::wants_write(flags);
        match self.authorized_path(&path, wants_write, 0) {
            Some(path) => {
                self.open_flags_and_finish(path, flags, mode, create_variant, wants_write)
            }
            None => Ok(-1),
        }
    }

    fn real_fs_mut(&mut self) -> &mut RealFs {
        self.real_fs
            .as_mut()
            .expect("try_real_filesystem_call only runs in real mode")
    }

    /// A host operation's `errno` outcome: `Some` on success, or `None` with
    /// `real_fs.errno` stamped — callers map `None` to the negative return.
    pub(super) fn hosted_errno<T>(&mut self, outcome: Result<T, i32>) -> Option<T> {
        match outcome {
            Ok(value) => Some(value),
            Err(errno) => {
                self.real_fs_mut().errno = errno;
                None
            }
        }
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
