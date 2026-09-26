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
mod path_calls;

use crate::interpreter::evaluator::{
    EvalResult, FilesystemGrantAccess, FilesystemGrantRefusal, FilesystemGrantRefusalReason,
    PreparedByteOutput, PreparedFilesystemCall, Value,
};
use crate::{
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRowKind, FilesystemAuthorizedPath,
    FilesystemGrantRootIdentity, FilesystemMetadataObservationKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Errno for real-mode failures, from the host `io::Error` when available.
fn io_errno(error: &std::io::Error) -> i32 {
    error.raw_os_error().unwrap_or(5) // EIO when the host gives no code
}

fn sponsor_value<T>(result: Result<T, crate::FilesystemSponsorError>) -> EvalResult<T> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => crate::interpreter::evaluator::filesystem_sponsor_halt(error),
    }
}

fn checked_written_count(written: usize) -> EvalResult<i64> {
    match i64::try_from(written) {
        Ok(written) => Ok(written),
        Err(_) => crate::interpreter::evaluator::filesystem_sponsor_halt(
            crate::FilesystemSponsorError::ArithmeticOverflow,
        ),
    }
}

/// A sponsor preflight can be absent, ready to commit, or can predict an
/// ordinary host-filesystem refusal. The last case still executes the host
/// operation so callers receive its native errno/BOOL; host success would
/// reveal sponsor/host divergence and is therefore an invariant halt.
#[derive(Debug)]
enum SponsorPreparation<T> {
    Unsponsored,
    Prepared(T),
    ExpectedHostFailure,
}

fn sponsor_preparation<T>(
    result: Result<T, crate::FilesystemSponsorError>,
) -> EvalResult<SponsorPreparation<T>> {
    match result {
        Ok(prepared) => Ok(SponsorPreparation::Prepared(prepared)),
        Err(
            crate::FilesystemSponsorError::ParentEntryMissing(_)
            | crate::FilesystemSponsorError::ParentIsNotDirectory(_)
            | crate::FilesystemSponsorError::EntryAlreadyExists(_)
            | crate::FilesystemSponsorError::EntryNotFound(_)
            | crate::FilesystemSponsorError::EntryIsNotRegularObject(_)
            | crate::FilesystemSponsorError::DirectoryNotEmpty(_)
            | crate::FilesystemSponsorError::InvalidDirectoryRename(_),
        ) => Ok(SponsorPreparation::ExpectedHostFailure),
        Err(error) => crate::interpreter::evaluator::filesystem_sponsor_halt(error),
    }
}

fn unexpected_sponsored_success<T>() -> EvalResult<T> {
    crate::interpreter::evaluator::filesystem_sponsor_halt(
        crate::FilesystemSponsorError::TransactionNoLongerCurrent,
    )
}

fn read_only_open_bypasses_sponsor(
    path: &Path,
    session_root: &Path,
    may_create: bool,
    truncates: bool,
    wants_write: bool,
) -> bool {
    (!path.starts_with(session_root) || path == session_root)
        && !wants_write
        && !may_create
        && !truncates
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

/// Canonicalized [`crate::FsGrants`]: the roots a scoped run may read/write
/// under, resolved once at construction so prefix checks compare real paths.
#[derive(Debug)]
struct Grants {
    read_roots: Vec<GrantRoot>,
    write_roots: Vec<GrantRoot>,
}

#[derive(Debug)]
struct GrantRoot {
    identity: FilesystemGrantRootIdentity,
    path: PathBuf,
    canonical_metadata: Option<CanonicalFilesystemMetadataIndex>,
}

/// Canonicalize a grant root before execution. Scoped evidence needs one exact
/// physical root for both authorization and rooted-path identity, so an absent
/// or unresolvable root is an invalid compiler grant rather than a path-level
/// errno observed by package code.
fn canonical_root(root: &Path) -> Result<PathBuf, String> {
    root.canonicalize().map_err(|error| {
        format!(
            "filesystem grant root `{}` cannot be resolved: {error}",
            root.display()
        )
    })
}

impl Grants {
    fn matching_root(&self, resolved: &Path, write: bool) -> Option<&GrantRoot> {
        // A write root grants read-back. Prefer the most specific root so a
        // build/output root nested under a source root keeps its output
        // identity. Grant validation rejects equal physical roots, so no
        // caller-order or numeric tie-break can select evidence identity.
        self.write_roots
            .iter()
            .chain(if write {
                [].iter()
            } else {
                self.read_roots.as_slice().iter()
            })
            .filter(|root| resolved.starts_with(&root.path))
            .max_by(|left, right| {
                left.path
                    .components()
                    .count()
                    .cmp(&right.path.components().count())
            })
    }

    fn root(&self, identity: FilesystemGrantRootIdentity) -> Option<&GrantRoot> {
        self.write_roots
            .iter()
            .chain(self.read_roots.iter())
            .find(|root| root.identity == identity)
    }
}

fn canonical_grants(grants: crate::FsGrants) -> Result<Grants, String> {
    let mut identities = BTreeSet::new();
    let mut physical_roots = BTreeMap::<PathBuf, FilesystemGrantRootIdentity>::new();
    let mut canonicalize =
        |root: crate::FilesystemGrantRoot, write: bool| -> Result<GrantRoot, String> {
            if write && root.canonical_metadata().is_some() {
                return Err(format!(
                    "writable filesystem grant root `{}` cannot carry immutable canonical metadata",
                    root.identity().get()
                ));
            }
            if !identities.insert(root.identity()) {
                return Err(format!(
                    "filesystem grant-root identity `{}` is duplicated",
                    root.identity().get()
                ));
            }
            let path = canonical_root(root.path())?;
            if let Some(previous) = physical_roots.insert(path.clone(), root.identity()) {
                return Err(format!(
                    "filesystem grant root `{}` has conflicting identities `{}` and `{}`",
                    path.display(),
                    previous.get(),
                    root.identity().get()
                ));
            }
            Ok(GrantRoot {
                identity: root.identity(),
                path,
                canonical_metadata: root.canonical_metadata().cloned(),
            })
        };
    let read_roots = grants
        .read_roots
        .into_iter()
        .map(|root| canonicalize(root, false))
        .collect::<Result<Vec<_>, _>>()?;
    let write_roots = grants
        .write_roots
        .into_iter()
        .map(|root| canonicalize(root, true))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Grants {
        read_roots,
        write_roots,
    })
}

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

fn real_path(bytes: &[u8]) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
    }
    #[cfg(not(unix))]
    {
        std::str::from_utf8(bytes).ok().map(PathBuf::from)
    }
}

fn real_os_byte_slice(value: &std::ffi::OsStr) -> Option<&[u8]> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(value.as_bytes())
    }
    #[cfg(not(unix))]
    {
        value.to_str().map(str::as_bytes)
    }
}

fn real_os_bytes(value: &std::ffi::OsStr) -> Option<Vec<u8>> {
    real_os_byte_slice(value).map(<[u8]>::to_vec)
}

#[derive(Clone, Copy)]
enum DirectoryEntrySnapshotKind {
    PackedRecords,
    FindCursor,
}

/// List a real directory as `(name, d_type)` entries: `.`/`..` first, then
/// immediate children sorted for deterministic interpreter behavior. The
/// selected consumer decides whether to reserve only retained names or also
/// the packed-dirent lane. Host failures remain errno results; compiler-owned
/// snapshot ceilings are evaluator resource halts.
fn real_directory_entries(
    path: &Path,
    snapshot_kind: DirectoryEntrySnapshotKind,
) -> EvalResult<Result<Vec<(Vec<u8>, u8)>, i32>> {
    match std::fs::metadata(path) {
        Ok(metadata) if !metadata.is_dir() => return Ok(Err(ENOTDIR)),
        Ok(_) => {}
        Err(error) => return Ok(Err(io_errno(&error))),
    }
    let (mut name_bytes, mut record_bytes) = match snapshot_kind {
        DirectoryEntrySnapshotKind::PackedRecords => {
            let record_bytes =
                crate::interpreter::evaluator::checked_directory_record_snapshot_total(0, 1)?;
            (
                None,
                Some(
                    crate::interpreter::evaluator::checked_directory_record_snapshot_total(
                        record_bytes,
                        2,
                    )?,
                ),
            )
        }
        DirectoryEntrySnapshotKind::FindCursor => {
            let name_bytes =
                crate::interpreter::evaluator::checked_directory_name_snapshot_total(0, 1)?;
            (
                Some(
                    crate::interpreter::evaluator::checked_directory_name_snapshot_total(
                        name_bytes, 2,
                    )?,
                ),
                None,
            )
        }
    };
    let mut children: Vec<(Vec<u8>, u8)> = Vec::new();
    let listing = match std::fs::read_dir(path) {
        Ok(listing) => listing,
        Err(error) => return Ok(Err(io_errno(&error))),
    };
    for dir_entry in listing {
        let dir_entry = match dir_entry {
            Ok(dir_entry) => dir_entry,
            Err(error) => return Ok(Err(io_errno(&error))),
        };
        let d_type = match dir_entry.file_type() {
            Ok(kind) if kind.is_dir() => 4,      // DT_DIR
            Ok(kind) if kind.is_symlink() => 10, // DT_LNK
            Ok(_) => 8,                          // DT_REG
            Err(_) => 0,                         // DT_UNKNOWN
        };
        let file_name = dir_entry.file_name();
        let Some(bytes) = real_os_byte_slice(&file_name) else {
            return Ok(Err(EINVAL));
        };
        let bytes = crate::interpreter::evaluator::portable_directory_entry_name(bytes);
        if let Some(current) = name_bytes {
            name_bytes = Some(
                crate::interpreter::evaluator::checked_directory_name_snapshot_total(
                    current,
                    bytes.len(),
                )?,
            );
        }
        if let Some(current) = record_bytes {
            record_bytes = Some(
                crate::interpreter::evaluator::checked_directory_record_snapshot_total(
                    current,
                    bytes.len(),
                )?,
            );
        }
        children.push((bytes.to_vec(), d_type));
    }
    children.sort();
    let mut entries: Vec<(Vec<u8>, u8)> = vec![(b".".to_vec(), 4), (b"..".to_vec(), 4)];
    entries.extend(children);
    Ok(Ok(entries))
}

/// `pread` emulation portable across hosts (std has no cross-platform
/// positioned read): seek to the offset, read, restore the cursor.
fn positioned_read(
    file: &mut std::fs::File,
    offset: i64,
    count: usize,
) -> std::io::Result<Vec<u8>> {
    let saved = file.stream_position()?;
    let offset = u64::try_from(offset.max(0)).expect("nonnegative i64 fits in u64");
    file.seek(SeekFrom::Start(offset))?;
    let mut buffer = vec![0u8; count];
    let outcome = file.read(&mut buffer);
    file.seek(SeekFrom::Start(saved))?;
    let n = outcome?;
    buffer.truncate(n);
    Ok(buffer)
}

/// `pwrite` emulation, mirroring [`positioned_read`].
fn positioned_write(file: &mut std::fs::File, offset: i64, bytes: &[u8]) -> std::io::Result<usize> {
    let saved = file.stream_position()?;
    let offset = u64::try_from(offset.max(0)).expect("nonnegative i64 fits in u64");
    file.seek(SeekFrom::Start(offset))?;
    let outcome = file.write(bytes);
    file.seek(SeekFrom::Start(saved))?;
    outcome
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
/// path itself when it exists, else canonicalize its parent and re-attach the
/// leaf (the create/new-file case). `None` when even the parent does not
/// resolve -- the caller reports ENOENT, exactly what the OS would say.
fn resolve_for_check(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = path.canonicalize() {
        return Some(canonical);
    }
    let parent = path.parent()?;
    let name = path.file_name()?;
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".").canonicalize().ok()?
    } else {
        parent.canonicalize().ok()?
    };
    Some(parent.join(name))
}

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

    /// Enforce the compiler grant retained by an opened descriptor before a
    /// descriptor-based mutation reaches sponsor accounting or the host.
    /// Missing descriptors preserve each ABI family's native error shape.
    fn require_real_descriptor_write_grant(&mut self, fd: i32, win32_errors: bool) -> bool {
        match self
            .real_fs
            .as_ref()
            .and_then(|filesystem| filesystem.descriptor_write_granted(fd))
        {
            Some(true) => true,
            Some(false) => {
                self.real_fs_mut().errno = if win32_errors { 5 } else { EACCES };
                false
            }
            None => {
                self.real_fs_mut().errno = if win32_errors { 6 } else { EBADF };
                false
            }
        }
    }

    fn prepare_sponsored_open(
        &mut self,
        path: &Path,
        may_create: bool,
        truncates: bool,
        wants_write: bool,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemOpen>> {
        let Some(sponsor) = self
            .real_fs
            .as_ref()
            .and_then(|filesystem| filesystem.sponsor.clone())
        else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        let session_root = sponsor_value(sponsor.session_root())?;
        if read_only_open_bypasses_sponsor(path, &session_root, may_create, truncates, wants_write)
        {
            return Ok(SponsorPreparation::Unsponsored);
        }
        let sponsored_path = sponsor_value(sponsor.bind_path(path))?;
        let entry = sponsor_value(sponsor.entry(&sponsored_path))?;
        if matches!(entry, Some(crate::FilesystemSponsorEntry::Directory))
            && !may_create
            && !truncates
        {
            return Ok(SponsorPreparation::Unsponsored);
        }
        let prepared = if entry.is_none() && may_create {
            sponsor.prepare_create_object_open(&sponsored_path, 0)
        } else {
            sponsor.prepare_open_with_extent(&sponsored_path, truncates.then_some(0))
        };
        sponsor_preparation(prepared)
    }

    fn finish_real_open(
        &mut self,
        opened: std::io::Result<std::fs::File>,
        path: PathBuf,
        append: bool,
        prepared: SponsorPreparation<crate::PreparedFilesystemOpen>,
        win32_errors: bool,
    ) -> EvalResult<i64> {
        match opened {
            Ok(file) => {
                let sponsor_descriptor = self.commit_sponsored_open(prepared)?;
                match self
                    .real_fs_mut()
                    .insert(file, path, sponsor_descriptor, append)
                {
                    Ok(fd) => Ok(fd),
                    Err(errno) => {
                        self.real_fs_mut().errno = if win32_errors { 5 } else { errno };
                        Ok(-1)
                    }
                }
            }
            Err(error) => {
                self.real_fs_mut().errno = if win32_errors {
                    win32_error_code(&error)
                } else {
                    io_errno(&error)
                };
                Ok(-1)
            }
        }
    }

    fn commit_sponsored_open(
        &mut self,
        prepared: SponsorPreparation<crate::PreparedFilesystemOpen>,
    ) -> EvalResult<Option<crate::FilesystemOpenDescriptor>> {
        match prepared {
            SponsorPreparation::Unsponsored => Ok(None),
            SponsorPreparation::Prepared(prepared) => Ok(Some(sponsor_value(prepared.commit())?)),
            SponsorPreparation::ExpectedHostFailure => unexpected_sponsored_success(),
        }
    }

    fn prepare_sponsored_duplicate(
        &mut self,
        fd: i32,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemOpen>> {
        let real = self.real_fs_mut();
        let Some(sponsor) = real.sponsor.clone() else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        let Some(descriptor) = real
            .files
            .get(&fd)
            .and_then(|entry| entry.sponsor_descriptor)
        else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        sponsor_preparation(sponsor.prepare_duplicate(&descriptor))
    }

    fn prepare_sponsored_close(
        &mut self,
        fd: i32,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let real = self.real_fs_mut();
        let Some(sponsor) = real.sponsor.clone() else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        let Some(descriptor) = real
            .files
            .get(&fd)
            .and_then(|entry| entry.sponsor_descriptor)
        else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        sponsor_preparation(sponsor.prepare_close(&descriptor))
    }

    fn prepare_sponsored_set_extent(
        &mut self,
        fd: i32,
        extent: u64,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let real = self.real_fs_mut();
        let Some(sponsor) = real.sponsor.clone() else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        let Some(entry) = real.files.get(&fd) else {
            return Ok(SponsorPreparation::ExpectedHostFailure);
        };
        let Some(descriptor) = entry.sponsor_descriptor else {
            return Ok(SponsorPreparation::ExpectedHostFailure);
        };
        sponsor_preparation(sponsor.prepare_set_extent(&descriptor, extent))
    }

    fn prepare_sponsored_write(
        &mut self,
        fd: i32,
        requested_bytes: usize,
        positioned_offset: Option<i64>,
    ) -> EvalResult<Result<SponsorPreparation<crate::PreparedFilesystemWrite>, i32>> {
        let real = self.real_fs_mut();
        let Some(sponsor) = real.sponsor.clone() else {
            return Ok(Ok(SponsorPreparation::Unsponsored));
        };
        let Some(entry) = real.files.get_mut(&fd) else {
            return Ok(Err(EBADF));
        };
        let Some(descriptor) = entry.sponsor_descriptor else {
            return Ok(Ok(SponsorPreparation::ExpectedHostFailure));
        };
        let offset = if entry.append {
            match entry.file.metadata() {
                Ok(metadata) => metadata.len(),
                Err(error) => return Ok(Err(io_errno(&error))),
            }
        } else if let Some(offset) = positioned_offset {
            u64::try_from(offset.max(0)).expect("nonnegative i64 fits in u64")
        } else {
            match entry.file.stream_position() {
                Ok(offset) => offset,
                Err(error) => return Ok(Err(io_errno(&error))),
            }
        };
        let requested_bytes = match u64::try_from(requested_bytes) {
            Ok(bytes) => bytes,
            Err(_) => {
                return crate::interpreter::evaluator::filesystem_sponsor_halt(
                    crate::FilesystemSponsorError::ArithmeticOverflow,
                );
            }
        };
        Ok(Ok(sponsor_preparation(sponsor.prepare_write(
            &descriptor,
            offset,
            requested_bytes,
        ))?))
    }

    fn commit_sponsored_write(
        &mut self,
        prepared: SponsorPreparation<crate::PreparedFilesystemWrite>,
        written: usize,
    ) -> EvalResult<()> {
        let prepared = match prepared {
            SponsorPreparation::Unsponsored => return Ok(()),
            SponsorPreparation::Prepared(prepared) => prepared,
            SponsorPreparation::ExpectedHostFailure => {
                return unexpected_sponsored_success();
            }
        };
        let written = match u64::try_from(written) {
            Ok(written) => written,
            Err(_) => {
                return crate::interpreter::evaluator::filesystem_sponsor_halt(
                    crate::FilesystemSponsorError::ArithmeticOverflow,
                );
            }
        };
        sponsor_value(prepared.commit_written(written))
    }

    fn prepare_sponsored_create_directory(
        &mut self,
        path: &Path,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some((sponsor, path)) = self.sponsor_path(path)? else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        sponsor_preparation(sponsor.prepare_create_directory(&path))
    }

    fn prepare_sponsored_unlink(
        &mut self,
        path: &Path,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some((sponsor, path)) = self.sponsor_path(path)? else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        sponsor_preparation(sponsor.prepare_unlink(&path))
    }

    fn prepare_sponsored_rename(
        &mut self,
        from: &Path,
        to: &Path,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some(sponsor) = self
            .real_fs
            .as_ref()
            .and_then(|filesystem| filesystem.sponsor.clone())
        else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        let from = sponsor_value(sponsor.bind_path(from))?;
        let to = sponsor_value(sponsor.bind_path(to))?;
        sponsor_preparation(sponsor.prepare_rename(&from, &to))
    }

    fn prepare_sponsored_hard_link(
        &mut self,
        existing: &Path,
        new_name: &Path,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some(sponsor) = self
            .real_fs
            .as_ref()
            .and_then(|filesystem| filesystem.sponsor.clone())
        else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        let existing = sponsor_value(sponsor.bind_path(existing))?;
        let new_name = sponsor_value(sponsor.bind_path(new_name))?;
        sponsor_preparation(sponsor.prepare_hard_link(&existing, &new_name))
    }

    fn prepare_sponsored_symlink(
        &mut self,
        link: &Path,
        target_spelling: &[u8],
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some((sponsor, link)) = self.sponsor_path(link)? else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        sponsor_preparation(sponsor.prepare_create_symlink(&link, target_spelling))
    }

    fn sponsor_path(
        &mut self,
        path: &Path,
    ) -> EvalResult<Option<(crate::FilesystemSponsor, crate::FilesystemSponsorPath)>> {
        let Some(sponsor) = self
            .real_fs
            .as_ref()
            .and_then(|filesystem| filesystem.sponsor.clone())
        else {
            return Ok(None);
        };
        let path = sponsor_value(sponsor.bind_path(path))?;
        Ok(Some((sponsor, path)))
    }

    fn commit_sponsored_mutation(
        &mut self,
        prepared: SponsorPreparation<crate::PreparedFilesystemMutation>,
    ) -> EvalResult<()> {
        match prepared {
            SponsorPreparation::Unsponsored => Ok(()),
            SponsorPreparation::Prepared(prepared) => sponsor_value(prepared.commit()),
            SponsorPreparation::ExpectedHostFailure => unexpected_sponsored_success(),
        }
    }

    fn finish_real_mutation(
        &mut self,
        outcome: std::io::Result<()>,
        prepared: SponsorPreparation<crate::PreparedFilesystemMutation>,
        win32_result: bool,
    ) -> EvalResult<i64> {
        match outcome {
            Ok(()) => {
                self.commit_sponsored_mutation(prepared)?;
                Ok(if win32_result { 1 } else { 0 })
            }
            Err(error) => {
                self.real_fs_mut().errno = if win32_result {
                    win32_error_code(&error)
                } else {
                    io_errno(&error)
                };
                Ok(if win32_result { 0 } else { -1 })
            }
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
            .push(FilesystemAuthorizedPath {
                operand_ordinal,
                access: if write {
                    FilesystemGrantAccess::Write
                } else {
                    FilesystemGrantAccess::Read
                },
                root,
                relative_path,
            });
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
            .push(FilesystemGrantRefusal {
                operand_ordinal,
                access: if write {
                    FilesystemGrantAccess::Write
                } else {
                    FilesystemGrantAccess::Read
                },
                reason,
            });
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

/// Open through `options`, serving DIRECTORIES too: a read-only open of a
/// directory (the `open_at`/`unlink_at`/`read_dir` dirfd mint) needs
/// FILE_FLAG_BACKUP_SEMANTICS on windows -- std's plain OpenOptions refuses
/// directory handles there, while unix serves `open(dir, O_RDONLY)` natively.
/// Write-intent opens are NOT redirected, so a write-open of a directory
/// fails on windows exactly like unix's EISDIR.
fn open_real(
    options: &std::fs::OpenOptions,
    path: &Path,
    wants_write: bool,
) -> std::io::Result<std::fs::File> {
    #[cfg(windows)]
    if !wants_write && path.is_dir() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        return std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path);
    }
    #[cfg(not(windows))]
    let _ = wants_write;
    options.open(path)
}

/// The shared OpenOptions decode for `open`/`open_create`/`open_at`: access
/// mode from the low bits, flag bits via the host mirror, and (unix,
/// open_create only) the creation mode.
fn open_options_for(flags: i32, mode: u32, apply_creation_mode: bool) -> std::fs::OpenOptions {
    let access = flags & 0x3;
    let mut options = std::fs::OpenOptions::new();
    options
        .read(access == 0 || access == 2)
        .write(access == 1 || access == 2)
        .append(crate::interpreter::evaluator::filesystem::host_open_flags::o_append(flags))
        .truncate(crate::interpreter::evaluator::filesystem::host_open_flags::o_trunc(flags))
        .create(crate::interpreter::evaluator::filesystem::host_open_flags::o_creat(flags));
    if crate::interpreter::evaluator::filesystem::host_open_flags::o_creat(flags)
        && crate::interpreter::evaluator::filesystem::host_open_flags::o_excl(flags)
    {
        options.create_new(true);
    }
    #[cfg(unix)]
    if crate::interpreter::evaluator::filesystem::host_open_flags::o_creat(flags)
        && apply_creation_mode
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode & 0o7777);
    }
    #[cfg(not(unix))]
    let _ = (mode, apply_creation_mode);
    options
}

/// `flock(fd, op)` against std's advisory file locks. LOCK_SH=1 LOCK_EX=2
/// LOCK_NB=4 LOCK_UN=8; a non-blocking miss is EWOULDBLOCK(35), matching the
/// virtual model.
fn real_lock(file: &std::fs::File, operation: i32, errno: &mut i32) -> i64 {
    const EWOULDBLOCK: i32 = 35;
    let non_blocking = operation & 4 != 0;
    let outcome = if operation & 8 != 0 {
        file.unlock()
    } else if operation & 2 != 0 {
        if non_blocking {
            match file.try_lock() {
                Ok(()) => Ok(()),
                Err(_) => {
                    *errno = EWOULDBLOCK;
                    return -1;
                }
            }
        } else {
            file.lock()
        }
    } else if operation & 1 != 0 {
        if non_blocking {
            match file.try_lock_shared() {
                Ok(()) => Ok(()),
                Err(_) => {
                    *errno = EWOULDBLOCK;
                    return -1;
                }
            }
        } else {
            file.lock_shared()
        }
    } else {
        *errno = 22; // EINVAL: no operation bit
        return -1;
    };
    match outcome {
        Ok(()) => 0,
        Err(error) => {
            *errno = io_errno(&error);
            -1
        }
    }
}

/// Win32 LockFileEx flags over std's portable file-lock API. Returns BOOL and
/// records Win32 ERROR_LOCK_VIOLATION (33) for a non-blocking contention miss.
fn real_lock_win32(file: &std::fs::File, flags: i32, last_error: &mut i32) -> i64 {
    let immediate = flags & 1 != 0;
    let exclusive = flags & 2 != 0;
    if immediate {
        let outcome = if exclusive {
            file.try_lock()
        } else {
            file.try_lock_shared()
        };
        return match outcome {
            Ok(()) => 1,
            Err(std::fs::TryLockError::WouldBlock) => {
                *last_error = 33;
                0
            }
            Err(std::fs::TryLockError::Error(error)) => {
                *last_error = error.raw_os_error().unwrap_or(1);
                0
            }
        };
    }
    let outcome = if exclusive {
        file.lock()
    } else {
        file.lock_shared()
    };
    match outcome {
        Ok(()) => 1,
        Err(error) => {
            *last_error = error.raw_os_error().unwrap_or(1);
            0
        }
    }
}

fn win32_error_code(error: &std::io::Error) -> i32 {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::NotFound => 2,
        ErrorKind::PermissionDenied => 5,
        ErrorKind::AlreadyExists => 183,
        ErrorKind::WouldBlock => 33,
        _ => error.raw_os_error().unwrap_or(1),
    }
}

#[cfg(test)]
mod sponsor_provider_tests;
