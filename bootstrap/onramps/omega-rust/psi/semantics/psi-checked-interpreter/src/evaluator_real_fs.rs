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

use super::{
    EvalResult, FilesystemGrantAccess, FilesystemGrantRefusal, FilesystemGrantRefusalReason,
    PreparedByteOutput, PreparedFilesystemCall, Value, host_open_flags, synthetic_handle_fd,
};
use crate::{FilesystemAuthorizedPath, FilesystemGrantRootIdentity};
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
        Err(error) => super::filesystem_sponsor_halt(error),
    }
}

fn checked_written_count(written: usize) -> EvalResult<i64> {
    match i64::try_from(written) {
        Ok(written) => Ok(written),
        Err(_) => super::filesystem_sponsor_halt(crate::FilesystemSponsorError::ArithmeticOverflow),
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
        Err(error) => super::filesystem_sponsor_halt(error),
    }
}

fn unexpected_sponsored_success<T>() -> EvalResult<T> {
    super::filesystem_sponsor_halt(crate::FilesystemSponsorError::TransactionNoLongerCurrent)
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
    let mut canonicalize = |root: crate::FilesystemGrantRoot| -> Result<GrantRoot, String> {
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
        })
    };
    let read_roots = grants
        .read_roots
        .into_iter()
        .map(&mut canonicalize)
        .collect::<Result<Vec<_>, _>>()?;
    let write_roots = grants
        .write_roots
        .into_iter()
        .map(canonicalize)
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
}

pub(super) struct RealFs {
    /// Synthetic fd -> real open file. Same table shape as `virtual_fds`;
    /// descriptors start at 3 (0/1/2 are the standard streams).
    files: BTreeMap<i32, RealFd>,
    next_fd: i32,
    /// Thread-local errno model, mirroring `virtual_errno`: set from the host
    /// `io::Error` on a failing op, read back by `errno`.
    pub(super) errno: i32,
    /// `Some` under `FilesystemAccess::RealScoped`; `None` is unscoped.
    grants: Option<Grants>,
    sponsor: Option<crate::FilesystemSponsor>,
}

impl RealFs {
    pub(super) fn new(
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

    pub(super) fn is_scoped(&self) -> bool {
        self.grants.is_some()
    }

    pub(super) fn rooted_path_bytes(
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
            },
        );
        i64::from(fd)
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

fn real_os_bytes(value: &std::ffi::OsStr) -> Option<Vec<u8>> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(value.as_bytes().to_vec())
    }
    #[cfg(not(unix))]
    {
        value.to_str().map(|value| value.as_bytes().to_vec())
    }
}

/// List a real directory as `(name, d_type)` entries for dirent packing:
/// `.`/`..` first (native getdirentries reports them; the wrapper's decode
/// skips them by name), then the immediate children SORTED for determinism --
/// native order is filesystem-defined, so no program may rely on it. Errors
/// map to the errno the caller reports: ENOTDIR for a non-directory fd,
/// else the host's own code.
fn real_dirent_entries(path: &Path) -> Result<Vec<(Vec<u8>, u8)>, i32> {
    match std::fs::metadata(path) {
        Ok(metadata) if !metadata.is_dir() => return Err(ENOTDIR),
        Ok(_) => {}
        Err(error) => return Err(io_errno(&error)),
    }
    let mut children: Vec<(Vec<u8>, u8)> = Vec::new();
    let listing = std::fs::read_dir(path).map_err(|error| io_errno(&error))?;
    for dir_entry in listing {
        let dir_entry = dir_entry.map_err(|error| io_errno(&error))?;
        let d_type = match dir_entry.file_type() {
            Ok(kind) if kind.is_dir() => 4,      // DT_DIR
            Ok(kind) if kind.is_symlink() => 10, // DT_LNK
            Ok(_) => 8,                          // DT_REG
            Err(_) => 0,                         // DT_UNKNOWN
        };
        let file_name = dir_entry.file_name();
        let bytes = real_os_bytes(&file_name).ok_or(EINVAL)?;
        children.push((bytes, d_type));
    }
    children.sort();
    let mut entries: Vec<(Vec<u8>, u8)> = vec![(b".".to_vec(), 4), (b"..".to_vec(), 4)];
    entries.extend(children);
    Ok(entries)
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

impl<'program> super::Evaluator<'program> {
    /// Mirror of `try_filesystem_call` against the REAL filesystem. The match
    /// exhaustively covers the same closed operation type as the virtual
    /// provider, so neither provider can silently omit a canonical operation.
    pub(super) fn try_real_filesystem_call(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<Value> {
        let result: i64 = match call {
            PreparedFilesystemCall::Create { path, mode: _ } => {
                // O_WRONLY|O_CREAT|O_TRUNC: create/truncate, writable.
                match self.authorized_path(&path, true, 0) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_open(&path, true, true, true)?;
                        let opened = std::fs::OpenOptions::new()
                            .write(true)
                            .create(true)
                            .truncate(true)
                            .open(&path);
                        self.finish_real_open(opened, path, false, prepared, false)?
                    }
                    None => -1,
                }
            }
            PreparedFilesystemCall::Open { path, flags } => {
                self.real_open_prepared(path, flags, 0, false)?
            }
            PreparedFilesystemCall::OpenCreate { path, flags, mode } => {
                // Flag bits decode via the same host-flag mirror the virtual
                // fs uses (host_open_flags -- the program was compiled for
                // `host()`, so the numerology matches).
                let access = flags & 0x3;
                let wants_write = access == 1
                    || access == 2
                    || host_open_flags::o_creat(flags)
                    || host_open_flags::o_trunc(flags)
                    || host_open_flags::o_append(flags);
                match self.authorized_path(&path, wants_write, 0) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_open(
                            &path,
                            host_open_flags::o_creat(flags),
                            host_open_flags::o_trunc(flags),
                            wants_write,
                        )?;
                        let options = open_options_for(flags, mode as u32, true);
                        let opened = open_real(&options, &path, wants_write);
                        self.finish_real_open(
                            opened,
                            path,
                            host_open_flags::o_append(flags),
                            prepared,
                            false,
                        )?
                    }
                    None => -1,
                }
            }
            PreparedFilesystemCall::OpenPathHandle {
                path,
                desired_access: _,
                share_mode: _,
                security_attributes: _,
                creation_disposition: _,
                flags_and_attributes: _,
                template_file: _,
            } => {
                // Real-mode model of CreateFileA's metadata/query use. The
                // shared helper adds FILE_FLAG_BACKUP_SEMANTICS for a directory
                // on Windows, so the same synthetic handle table serves files
                // and directories.
                match self.authorized_path(&path, false, 0) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_open(&path, false, false, false)?;
                        let mut options = std::fs::OpenOptions::new();
                        options.read(true);
                        let opened = open_real(&options, &path, false);
                        self.finish_real_open(opened, path, false, prepared, true)?
                    }
                    None => {
                        let real = self.real_fs_mut();
                        real.errno = if real.errno == ENOENT { 2 } else { 5 };
                        -1
                    }
                }
            }
            PreparedFilesystemCall::Read { fd, buffer, count } => {
                let outcome = {
                    let real = self.real_fs_mut();
                    match real.files.get_mut(&fd) {
                        Some(entry) => {
                            let mut bytes = vec![0u8; count.host];
                            match entry.file.read(&mut bytes) {
                                Ok(n) => {
                                    bytes.truncate(n);
                                    Ok(bytes)
                                }
                                Err(error) => Err(io_errno(&error)),
                            }
                        }
                        None => Err(EBADF),
                    }
                };
                match outcome {
                    Ok(bytes) => {
                        let n = bytes.len() as i64;
                        buffer.write(&bytes)?;
                        n
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::Write { fd, bytes } => {
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(Value::Int(-1));
                }
                let prepared = match self.prepare_sponsored_write(fd, bytes.len(), None)? {
                    Ok(prepared) => prepared,
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        return Ok(Value::Int(-1));
                    }
                };
                let real = self.real_fs_mut();
                let outcome = match real.files.get_mut(&fd) {
                    Some(entry) => match entry.file.write(&bytes) {
                        Ok(n) => Ok(n),
                        Err(error) => Err(io_errno(&error)),
                    },
                    None => Err(EBADF),
                };
                match outcome {
                    Ok(written) => {
                        self.commit_sponsored_write(prepared, written)?;
                        checked_written_count(written)?
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::Seek { fd, offset, whence } => {
                let position = match whence {
                    1 => SeekFrom::Current(offset),
                    2 => SeekFrom::End(offset),
                    _ => SeekFrom::Start(offset.max(0) as u64),
                };
                let real = self.real_fs_mut();
                match real.files.get_mut(&fd) {
                    Some(entry) => match entry.file.seek(position) {
                        Ok(new_position) => new_position as i64,
                        Err(error) => {
                            real.errno = io_errno(&error);
                            -1
                        }
                    },
                    None => {
                        real.errno = EBADF;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::Close { fd } => {
                if self.real_fs_mut().files.contains_key(&fd) {
                    let prepared = self.prepare_sponsored_close(fd)?;
                    self.real_fs_mut().files.remove(&fd);
                    self.commit_sponsored_mutation(prepared)?;
                    0 // the File drop closes the real descriptor
                } else {
                    self.real_fs_mut().errno = EBADF;
                    -1
                }
            }
            PreparedFilesystemCall::CloseHandle { handle } => {
                match synthetic_handle_fd(handle) {
                    Some(handle) if self.real_fs_mut().files.contains_key(&handle) => {
                        let prepared = self.prepare_sponsored_close(handle)?;
                        self.real_fs_mut().files.remove(&handle);
                        self.commit_sponsored_mutation(prepared)?;
                        1 // Win32 BOOL success; dropping File closes the handle.
                    }
                    _ => {
                        self.real_fs_mut().errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                }
            }
            PreparedFilesystemCall::Duplicate { fd } => {
                let prepared = self.prepare_sponsored_duplicate(fd)?;
                let cloned = match self.real_fs_mut().files.get(&fd) {
                    Some(entry) => entry
                        .file
                        .try_clone()
                        .map(|file| (file, entry.path.clone(), entry.append))
                        .map_err(|error| io_errno(&error)),
                    None => Err(EBADF),
                };
                match cloned {
                    Ok((file, path, append)) => {
                        let sponsor_descriptor = self.commit_sponsored_open(prepared)?;
                        self.real_fs_mut()
                            .insert(file, path, sponsor_descriptor, append)
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::SetLen { fd, length } => {
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(Value::Int(-1));
                }
                let length = u64::try_from(length.max(0)).expect("nonnegative i64 fits in u64");
                let prepared = self.prepare_sponsored_set_extent(fd, length)?;
                let outcome = match self.real_fs_mut().files.get_mut(&fd) {
                    Some(entry) => entry.file.set_len(length).map_err(|error| io_errno(&error)),
                    None => Err(EBADF),
                };
                match outcome {
                    Ok(()) => {
                        self.commit_sponsored_mutation(prepared)?;
                        0
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::Sync { fd } | PreparedFilesystemCall::SyncData { fd } => {
                let real = self.real_fs_mut();
                match real.files.get_mut(&fd) {
                    Some(entry) => match entry.file.sync_all() {
                        Ok(()) => 0,
                        Err(error) => {
                            real.errno = io_errno(&error);
                            -1
                        }
                    },
                    None => {
                        real.errno = EBADF;
                        -1
                    }
                }
            }
            // `remove_name` is the TRUSTED plain-path twin (D-at trust class):
            // the arg bytes ARE the path, so both spellings share one arm.
            PreparedFilesystemCall::Remove { path }
            | PreparedFilesystemCall::RemoveName { path } => {
                match self.authorized_namespace_leaf(&path, true, 0) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_unlink(&path)?;
                        let outcome = std::fs::remove_file(path);
                        self.finish_real_mutation(outcome, prepared, false)?
                    }
                    None => -1,
                }
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
            PreparedFilesystemCall::RemoveDir { path }
            | PreparedFilesystemCall::RemoveDirName { path } => {
                match self.authorized_namespace_leaf(&path, true, 0) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_unlink(&path)?;
                        let outcome = std::fs::remove_dir(path);
                        self.finish_real_mutation(outcome, prepared, false)?
                    }
                    None => -1,
                }
            }
            PreparedFilesystemCall::Rename { from, to } => {
                // BOTH ends need write authority: a rename removes `from` and
                // creates `to`.
                match (
                    self.authorized_namespace_leaf(&from, true, 0),
                    self.authorized_namespace_leaf(&to, true, 1),
                ) {
                    (Some(from), Some(to)) => {
                        let prepared = self.prepare_sponsored_rename(&from, &to)?;
                        let outcome = std::fs::rename(from, to);
                        self.finish_real_mutation(outcome, prepared, false)?
                    }
                    _ => -1,
                }
            }
            PreparedFilesystemCall::ReadAt {
                fd,
                buffer,
                count,
                offset,
            } => {
                // `pread(fd, buf, count, offset)`: read at an absolute offset
                // WITHOUT moving the cursor. Emulated portably (std has no
                // cross-platform pread): seek, read, restore.
                let outcome = {
                    let real = self.real_fs_mut();
                    match real.files.get_mut(&fd) {
                        Some(entry) => positioned_read(&mut entry.file, offset, count.host)
                            .map_err(|error| io_errno(&error)),
                        None => Err(EBADF),
                    }
                };
                match outcome {
                    Ok(bytes) => {
                        let n = bytes.len() as i64;
                        buffer.write(&bytes)?;
                        n
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::WriteAt { fd, bytes, offset } => {
                // `pwrite(fd, buf, offset)`: write at an absolute offset
                // WITHOUT moving the cursor (same emulation).
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(Value::Int(-1));
                }
                let prepared = match self.prepare_sponsored_write(fd, bytes.len(), Some(offset))? {
                    Ok(prepared) => prepared,
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        return Ok(Value::Int(-1));
                    }
                };
                let outcome = match self.real_fs_mut().files.get_mut(&fd) {
                    Some(entry) => match positioned_write(&mut entry.file, offset, &bytes) {
                        Ok(n) => Ok(n),
                        Err(error) => Err(io_errno(&error)),
                    },
                    None => Err(EBADF),
                };
                match outcome {
                    Ok(written) => {
                        self.commit_sponsored_write(prepared, written)?;
                        checked_written_count(written)?
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::ReadDir {
                fd,
                buffer,
                count,
                position,
            } => {
                // `read_dir(fd, buf, count, &position)` -- the virtual
                // dispatcher's contract, mirrored. Pack `.`/`..` plus immediate
                // children as Darwin dirent records and return the next window
                // of complete records. The synthetic byte cursor lets repeated
                // calls drain directories larger than one caller buffer. Names
                // come from `std::fs::read_dir` and are sorted for determinism;
                // native getdirentries order remains filesystem-defined.
                let listed = {
                    let real = self.real_fs_mut();
                    match real.files.get(&fd) {
                        Some(entry) => real_dirent_entries(&entry.path),
                        None => Err(EBADF),
                    }
                };
                match listed {
                    Ok(entries) => {
                        let records = super::pack_dirent_records(&entries);
                        let start = position.initial.max(0) as usize;
                        let (chunk, next_position) =
                            super::dirent_record_chunk(&records, start, count.host);
                        if chunk.is_empty() {
                            0
                        } else {
                            let n = chunk.len();
                            buffer.write(chunk)?;
                            position.write(next_position as i64)?;
                            n as i64
                        }
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::FindFirst { pattern, data } => {
                // `find_first(pattern, &data)` -- the windows dir-walk seam
                // (fs rung 3a) served against the real filesystem: strip the
                // `/*` tail (the impl joins with `/`, which Win32 accepts),
                // list the directory (the same dot-prefixed sorted set
                // read_dir packs), snapshot the tail into a cursor keyed by a
                // fresh handle, and fill the FIRST entry's find-data record.
                let listed = match pattern.strip_suffix(b"/*") {
                    Some(dir_path) => match self.authorized_path(dir_path, false, 0) {
                        Some(path) => real_dirent_entries(&path),
                        None => {
                            return Ok(Value::Int(-1));
                        }
                    },
                    None => Err(ENOENT),
                };
                match listed {
                    Ok(entries) => {
                        let mut queue: std::collections::VecDeque<(Vec<u8>, bool)> = entries
                            .into_iter()
                            .map(|(name, d_type)| (name, d_type == 4))
                            .collect();
                        let (name, is_dir) =
                            queue.pop_front().expect("dot entries are always present");
                        self.write_find_data(&data, &name, is_dir)?;
                        let handle = self.virtual_next_find;
                        self.virtual_next_find += 1;
                        self.virtual_finds.insert(handle, queue);
                        handle
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::FindNext { handle, data } => {
                // Cursor-only (the snapshot was taken at find_first) -- the
                // same arm shape as the hermetic dispatcher.
                match self
                    .virtual_finds
                    .get_mut(&handle)
                    .and_then(std::collections::VecDeque::pop_front)
                {
                    Some((name, is_dir)) => {
                        self.write_find_data(&data, &name, is_dir)?;
                        1
                    }
                    None => 0,
                }
            }
            PreparedFilesystemCall::FindClose { handle } => {
                if self.virtual_finds.remove(&handle).is_some() {
                    1
                } else {
                    0
                }
            }
            PreparedFilesystemCall::ReadMetadata { path, buffer } => {
                self.real_read_metadata(path, &buffer, false)?
            }
            PreparedFilesystemCall::ReadSymlinkMetadata { path, buffer } => {
                self.real_read_metadata(path, &buffer, true)?
            }
            PreparedFilesystemCall::ReadFileMetadata { fd, buffer } => {
                let looked_up = match self.real_fs_mut().files.get(&fd) {
                    Some(entry) => entry.file.metadata().map_err(|error| io_errno(&error)),
                    None => Err(EBADF),
                };
                match looked_up {
                    Ok(metadata) => {
                        self.write_real_fs_stat(&buffer, &metadata)?;
                        0
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::Errno => i64::from(self.real_fs_mut().errno),
            PreparedFilesystemCall::Canonicalize { path, buffer } => {
                // `realpath(path, buf)`: NUL-terminated resolved path into the
                // buffer; non-zero success flag, 0 (NULL) + errno on failure --
                // the virtual contract's shape.
                match self.authorized_path(&path, false, 0) {
                    Some(path) => match std::fs::canonicalize(&path) {
                        Ok(resolved) => {
                            let Some(mut bytes) = real_os_bytes(resolved.as_os_str()) else {
                                self.real_fs_mut().errno = EINVAL;
                                return Ok(Value::Int(0));
                            };
                            bytes.push(0);
                            buffer.write(&bytes)?;
                            1
                        }
                        Err(error) => {
                            self.real_fs_mut().errno = io_errno(&error);
                            0
                        }
                    },
                    None => 0,
                }
            }
            PreparedFilesystemCall::HardLink { original, link } => {
                // `link(original, link)`: real inodes, unlike the virtual
                // byte-copy approximation. Both names require write authority:
                // linking a read-only source object into writable staging would
                // let later writes mutate the source through the shared inode.
                match (
                    self.authorized_namespace_leaf(&original, true, 0),
                    self.authorized_namespace_leaf(&link, true, 1),
                ) {
                    (Some(original), Some(link)) => {
                        let prepared = self.prepare_sponsored_hard_link(&original, &link)?;
                        let outcome = std::fs::hard_link(original, link);
                        self.finish_real_mutation(outcome, prepared, false)?
                    }
                    _ => -1,
                }
            }
            PreparedFilesystemCall::CreateHardLink {
                link,
                existing,
                security_attributes: _,
            } => {
                // `CreateHardLinkA(link, existing, security)` -- the windows
                // primitive's arg order (NEW link first) and BOOL result
                // (1 success / 0 failure). Served portably via std like
                // `hard_link` above; errno doubles as this provider's modeled
                // GetLastError slot and therefore stores Win32 codes here.
                match (
                    self.authorized_namespace_leaf(&existing, true, 1),
                    self.authorized_namespace_leaf(&link, true, 0),
                ) {
                    (Some(existing), Some(link)) => {
                        let prepared = self.prepare_sponsored_hard_link(&existing, &link)?;
                        let outcome = std::fs::hard_link(existing, link);
                        self.finish_real_mutation(outcome, prepared, true)?
                    }
                    _ => 0,
                }
            }
            PreparedFilesystemCall::GetOsfHandle { fd } => {
                // The fd -> HANDLE bridge (session slice 4a). The real
                // provider's files ride std::fs behind SYNTHETIC fds by
                // design (no raw handles), so its handles are the fds
                // themselves -- identity, like the hermetic model; -2 for an
                // unknown fd (msvcrt's bad-fd spelling).
                if self.real_fs_mut().files.contains_key(&fd) {
                    i64::from(fd)
                } else {
                    -2
                }
            }
            PreparedFilesystemCall::FinalPathNameByHandle {
                handle,
                buffer,
                capacity,
                flags: _,
            } => {
                // Resolve an open handle (= synthetic fd) to its final path:
                // std::fs::canonicalize of the entry's stored path (on a
                // windows host that IS the \\?\-prefixed final path, matching
                // native GetFinalPathNameByHandleA). Win32 return contract:
                // length without the NUL when it fits, required size with the
                // NUL when too small, 0 on failure; errno is this provider's
                // modeled GetLastError slot.
                let path = synthetic_handle_fd(handle).and_then(|handle| {
                    self.real_fs_mut()
                        .files
                        .get(&handle)
                        .map(|entry| entry.path.clone())
                });
                match path {
                    Some(path) => match std::fs::canonicalize(path) {
                        Ok(path) => {
                            let Some(path) = real_os_bytes(path.as_os_str()) else {
                                self.real_fs_mut().errno = 1113; // ERROR_NO_UNICODE_TRANSLATION
                                return Ok(Value::Int(0));
                            };
                            if path.len() < capacity.host {
                                let mut bytes = path.clone();
                                bytes.push(0);
                                buffer.write(&bytes)?;
                                path.len() as i64
                            } else {
                                (path.len() + 1) as i64
                            }
                        }
                        Err(error) => {
                            self.real_fs_mut().errno = win32_error_code(&error);
                            0
                        }
                    },
                    None => {
                        self.real_fs_mut().errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                }
            }
            PreparedFilesystemCall::SetFileTime {
                handle,
                creation: _,
                last_access: _,
                last_write: write_ft,
            } => {
                // `SetFileTime(handle, creation, access_ft, write_ft)` (session
                // slice 4b): apply the WRITE time from its FILETIME buffer via
                // std's set_modified, like `set_file_times` above. BOOL result;
                // 0 for a bad handle or a failed stamp; errno models
                // GetLastError for the wrapper's immediate capture.
                let filetime = write_ft
                    .get(0..8)
                    .and_then(|s| <[u8; 8]>::try_from(s).ok())
                    .map(i64::from_le_bytes)
                    .unwrap_or(0);
                let secs = filetime / 10_000_000 - 11_644_473_600;
                let Some(handle) = synthetic_handle_fd(handle) else {
                    self.real_fs_mut().errno = 6; // ERROR_INVALID_HANDLE
                    return Ok(Value::Int(0));
                };
                if !self.require_real_descriptor_write_grant(handle, true) {
                    return Ok(Value::Int(0));
                }
                let real = self.real_fs_mut();
                match real.files.get_mut(&handle) {
                    Some(entry) => {
                        let stamp = if secs >= 0 {
                            std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
                        } else {
                            std::time::UNIX_EPOCH
                        };
                        match entry.file.set_modified(stamp) {
                            Ok(()) => 1,
                            Err(error) => {
                                real.errno = win32_error_code(&error);
                                0
                            }
                        }
                    }
                    None => {
                        real.errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                }
            }
            PreparedFilesystemCall::Symlink { target, link } => {
                // `symlink(target, linkpath)`: the TARGET is stored verbatim
                // (never dereferenced here), so only the link path needs write
                // authority. Unix-only in std; elsewhere ENOTSUP.
                match self.authorized_namespace_leaf(&link, true, 1) {
                    Some(link) => {
                        let prepared = self.prepare_sponsored_symlink(&link, &target)?;
                        #[cfg(unix)]
                        {
                            let outcome = std::os::unix::fs::symlink(
                                real_path(&target).expect("unix path bytes are lossless"),
                                link,
                            );
                            self.finish_real_mutation(outcome, prepared, false)?
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (target, link, prepared);
                            self.real_fs_mut().errno = ENOTSUP;
                            -1
                        }
                    }
                    None => -1,
                }
            }
            PreparedFilesystemCall::ReadLink {
                path,
                buffer,
                count,
            } => {
                // `readlink(path, buf, count)`: target bytes into the buffer,
                // returns the count written.
                match self.authorized_path_no_follow(&path, false, 0) {
                    Some(path) => match std::fs::read_link(&path) {
                        Ok(target) => {
                            let Some(bytes) = real_os_bytes(target.as_os_str()) else {
                                self.real_fs_mut().errno = EINVAL;
                                return Ok(Value::Int(-1));
                            };
                            let n = bytes.len().min(count.host);
                            buffer.write(&bytes[..n])?;
                            n as i64
                        }
                        Err(error) => {
                            self.real_fs_mut().errno = io_errno(&error);
                            -1
                        }
                    },
                    None => -1,
                }
            }
            PreparedFilesystemCall::SetPermissions { path, mode } => {
                // `chmod(path, mode)`: metadata mutation = write authority.
                match self.authorized_path(&path, true, 0) {
                    Some(path) => {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            self.real_result_unit(std::fs::set_permissions(
                                path,
                                std::fs::Permissions::from_mode(mode & 0o7777),
                            ))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (path, mode);
                            self.real_fs_mut().errno = ENOTSUP;
                            -1
                        }
                    }
                    None => -1,
                }
            }
            PreparedFilesystemCall::SetFilePermissions { fd, mode } => {
                // `fchmod(fd, mode)`: the descriptor must retain write grant
                // even when the host permits metadata changes on a read-only
                // open file description.
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(Value::Int(-1));
                }
                let real = self.real_fs_mut();
                match real.files.get_mut(&fd) {
                    Some(entry) => {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            match entry
                                .file
                                .set_permissions(std::fs::Permissions::from_mode(mode & 0o7777))
                            {
                                Ok(()) => 0,
                                Err(error) => {
                                    real.errno = io_errno(&error);
                                    -1
                                }
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (entry, mode);
                            real.errno = ENOTSUP;
                            -1
                        }
                    }
                    None => {
                        real.errno = EBADF;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::SetFileTimes { fd, times } => {
                // `futimens(fd, times)`: two packed timespecs (atime, mtime);
                // the model (virtual and real alike) applies the MODIFIED time
                // -- times[1].tv_sec at byte offset 16.
                let mtime_secs = times
                    .bytes
                    .get(16..24)
                    .map(|bytes| i64::from_le_bytes(bytes.try_into().unwrap()))
                    .unwrap_or(0);
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(Value::Int(-1));
                }
                let real = self.real_fs_mut();
                match real.files.get_mut(&fd) {
                    Some(entry) => {
                        let stamp = if mtime_secs >= 0 {
                            std::time::UNIX_EPOCH
                                + std::time::Duration::from_secs(mtime_secs as u64)
                        } else {
                            std::time::UNIX_EPOCH
                        };
                        match entry.file.set_modified(stamp) {
                            Ok(()) => 0,
                            Err(error) => {
                                real.errno = io_errno(&error);
                                -1
                            }
                        }
                    }
                    None => {
                        real.errno = EBADF;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::LockFile { fd, operation } => {
                // `flock(fd, op)`: LOCK_SH=1 LOCK_EX=2 LOCK_NB=4 LOCK_UN=8,
                // served by std's advisory file locks on the real handle.
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(Value::Int(-1));
                }
                let real = self.real_fs_mut();
                match real.files.get(&fd) {
                    Some(entry) => real_lock(&entry.file, operation, &mut real.errno),
                    None => {
                        real.errno = EBADF;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::LockFileEx {
                handle,
                flags,
                reserved: _,
                length_low: _,
                length_high: _,
                overlapped: _,
            } => {
                // Win32 LockFileEx semantics over the provider's synthetic
                // handle. The exact byte range is intentionally ignored here:
                // the std wrapper always supplies offset zero + u64::MAX.
                let Some(fd) = synthetic_handle_fd(handle) else {
                    self.real_fs_mut().errno = 6; // ERROR_INVALID_HANDLE
                    return Ok(Value::Int(0));
                };
                if !self.require_real_descriptor_write_grant(fd, true) {
                    return Ok(Value::Int(0));
                }
                let flags = flags as i32;
                let real = self.real_fs_mut();
                match real.files.get(&fd) {
                    Some(entry) => real_lock_win32(&entry.file, flags, &mut real.errno),
                    None => {
                        real.errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                }
            }
            PreparedFilesystemCall::UnlockFile {
                handle,
                offset_low: _,
                offset_high: _,
                length_low: _,
                length_high: _,
            } => {
                let Some(fd) = synthetic_handle_fd(handle) else {
                    self.real_fs_mut().errno = 6; // ERROR_INVALID_HANDLE
                    return Ok(Value::Int(0));
                };
                if !self.require_real_descriptor_write_grant(fd, true) {
                    return Ok(Value::Int(0));
                }
                let real = self.real_fs_mut();
                match real.files.get(&fd) {
                    Some(entry) => match entry.file.unlock() {
                        Ok(()) => 1,
                        Err(error) => {
                            real.errno = error.raw_os_error().unwrap_or(158);
                            0
                        }
                    },
                    None => {
                        real.errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                }
            }
            PreparedFilesystemCall::GetLastError => i64::from(self.real_fs_mut().errno),
            PreparedFilesystemCall::ChangeOwner { path, uid, gid } => {
                // `chown`/`lchown(path, uid, gid)`: -1 leaves the component
                // alone (None). Metadata mutation = write authority.
                let authorized = self.authorized_path(&path, true, 0);
                match authorized {
                    Some(path) => {
                        #[cfg(unix)]
                        {
                            let owner = (uid >= 0).then_some(uid as u32);
                            let group = (gid >= 0).then_some(gid as u32);
                            let outcome = std::os::unix::fs::chown(path, owner, group);
                            self.real_result_unit(outcome)
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (path, uid, gid);
                            self.real_fs_mut().errno = ENOTSUP;
                            -1
                        }
                    }
                    None => -1,
                }
            }
            PreparedFilesystemCall::ChangeOwnerNoFollow { path, uid, gid } => {
                self.real_change_owner_no_follow(path, uid, gid)
            }
            PreparedFilesystemCall::ChangeFileOwner { fd, uid, gid } => {
                // `fchown(fd, uid, gid)`: by descriptor.
                if !self.require_real_descriptor_write_grant(fd, false) {
                    return Ok(Value::Int(-1));
                }
                let real = self.real_fs_mut();
                match real.files.get(&fd) {
                    Some(entry) => {
                        #[cfg(unix)]
                        {
                            let owner = (uid >= 0).then_some(uid as u32);
                            let group = (gid >= 0).then_some(gid as u32);
                            match std::os::unix::fs::fchown(&entry.file, owner, group) {
                                Ok(()) => 0,
                                Err(error) => {
                                    real.errno = io_errno(&error);
                                    -1
                                }
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (entry, uid, gid);
                            real.errno = ENOTSUP;
                            -1
                        }
                    }
                    None => {
                        real.errno = EBADF;
                        -1
                    }
                }
            }
            PreparedFilesystemCall::UnlinkAt { dirfd, name, flags } => {
                // `unlinkat(dirfd, name, flags)`: resolve against the dirfd's
                // OPENED path (the same trick read_dir rides -- std has no fd
                // relative ops); flags & AT_REMOVEDIR(0x80) removes a dir.
                let joined = match self.real_fs_mut().files.get(&dirfd) {
                    Some(entry) => match real_path(&name) {
                        Some(name) => entry.path.join(name),
                        None => {
                            self.real_fs_mut().errno = EACCES;
                            self.record_grant_refusal(
                                1,
                                true,
                                FilesystemGrantRefusalReason::UnrepresentableRootedPath,
                            );
                            return Ok(Value::Int(-1));
                        }
                    },
                    None => {
                        self.real_fs_mut().errno = EBADF;
                        return Ok(Value::Int(-1));
                    }
                };
                match self.authorized_native_path(&joined, true, false, 1) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_unlink(&path)?;
                        if flags & 0x80 != 0 {
                            let outcome = std::fs::remove_dir(path);
                            self.finish_real_mutation(outcome, prepared, false)?
                        } else {
                            let outcome = std::fs::remove_file(path);
                            self.finish_real_mutation(outcome, prepared, false)?
                        }
                    }
                    None => -1,
                }
            }
            PreparedFilesystemCall::OpenAt { dirfd, name, flags } => {
                // `openat(dirfd, name, flags)`: join against the dirfd's opened
                // path, then the ordinary open (same flag decode + grants).
                let access = flags & 0x3;
                let wants_write = access == 1
                    || access == 2
                    || host_open_flags::o_creat(flags)
                    || host_open_flags::o_trunc(flags)
                    || host_open_flags::o_append(flags);
                let joined = match self.real_fs_mut().files.get(&dirfd) {
                    Some(entry) => match real_path(&name) {
                        Some(name) => entry.path.join(name),
                        None => {
                            self.real_fs_mut().errno = EACCES;
                            self.record_grant_refusal(
                                1,
                                wants_write,
                                FilesystemGrantRefusalReason::UnrepresentableRootedPath,
                            );
                            return Ok(Value::Int(-1));
                        }
                    },
                    None => {
                        self.real_fs_mut().errno = EBADF;
                        return Ok(Value::Int(-1));
                    }
                };
                match self.authorized_native_path(&joined, wants_write, true, 1) {
                    Some(path) => {
                        let prepared = self.prepare_sponsored_open(
                            &path,
                            host_open_flags::o_creat(flags),
                            host_open_flags::o_trunc(flags),
                            wants_write,
                        )?;
                        let options = open_options_for(flags, 0, false);
                        let opened = open_real(&options, &path, wants_write);
                        self.finish_real_open(
                            opened,
                            path,
                            host_open_flags::o_append(flags),
                            prepared,
                            false,
                        )?
                    }
                    None => -1,
                }
            }
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
            || host_open_flags::o_creat(flags)
            || host_open_flags::o_trunc(flags)
            || host_open_flags::o_append(flags);
        match self.authorized_path(&path, wants_write, 0) {
            Some(path) => {
                let prepared = self.prepare_sponsored_open(
                    &path,
                    host_open_flags::o_creat(flags),
                    host_open_flags::o_trunc(flags),
                    wants_write,
                )?;
                let options = open_options_for(flags, mode, create_variant);
                let opened = open_real(&options, &path, wants_write);
                self.finish_real_open(
                    opened,
                    path,
                    host_open_flags::o_append(flags),
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
                Ok(self
                    .real_fs_mut()
                    .insert(file, path, sponsor_descriptor, append))
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
                return super::filesystem_sponsor_halt(
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
                return super::filesystem_sponsor_halt(
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
        no_follow: bool,
    ) -> EvalResult<i64> {
        let authorized = if no_follow {
            self.authorized_path_no_follow(&path, false, 0)
        } else {
            self.authorized_path(&path, false, 0)
        };
        let Some(path) = authorized else {
            return Ok(-1);
        };
        let looked_up = if no_follow {
            std::fs::symlink_metadata(path)
        } else {
            std::fs::metadata(path)
        };
        match looked_up {
            Ok(metadata) => {
                self.write_real_fs_stat(buffer, &metadata)?;
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

    fn real_result_unit(&mut self, outcome: std::io::Result<()>) -> i64 {
        match outcome {
            Ok(()) => 0,
            Err(error) => {
                self.real_fs_mut().errno = io_errno(&error);
                -1
            }
        }
    }

    /// Fill the caller's stat buffer at the HOST offsets from real
    /// `std::fs::Metadata` -- mode + size + mtime ride the shared
    /// `write_fs_stat` layout writer, so real and virtual mode lay out the
    /// same three fields at the same offsets (the wrapper's `decode_metadata`
    /// reads only those in rung 1's consumers).
    fn write_real_fs_stat(
        &self,
        output: &PreparedByteOutput,
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
        self.write_fs_stat(output, mode, size, mtime_secs)
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
        .append(host_open_flags::o_append(flags))
        .truncate(host_open_flags::o_trunc(flags))
        .create(host_open_flags::o_creat(flags));
    if host_open_flags::o_creat(flags) && host_open_flags::o_excl(flags) {
        options.create_new(true);
    }
    #[cfg(unix)]
    if host_open_flags::o_creat(flags) && apply_creation_mode {
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
mod sponsor_provider_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let identity = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "omega-real-fs-{label}-{}-{identity}",
                std::process::id()
            ));
            std::fs::create_dir(&path).expect("create provider test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_grant_root(identity: u32, path: PathBuf) -> crate::FilesystemGrantRoot {
        crate::FilesystemGrantRoot::new(
            crate::FilesystemGrantRootIdentity::new(identity)
                .expect("test grant identity is nonzero"),
            path,
        )
    }

    #[test]
    fn grant_root_identities_and_physical_roots_are_unambiguous() {
        let directory = TestDirectory::new("grant-identities");
        let source = directory.0.join("source");
        let output = directory.0.join("output");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&output).unwrap();

        let duplicate_identity = canonical_grants(crate::FsGrants {
            read_roots: vec![test_grant_root(1, source.clone())],
            write_roots: vec![test_grant_root(1, output.clone())],
        })
        .expect_err("one evidence identity cannot name two roots");
        assert!(duplicate_identity.contains("identity `1` is duplicated"));

        let duplicate_physical = canonical_grants(crate::FsGrants {
            read_roots: vec![test_grant_root(1, source.clone())],
            write_roots: vec![test_grant_root(2, source)],
        })
        .expect_err("one physical root cannot carry two evidence identities");
        assert!(duplicate_physical.contains("conflicting identities `1` and `2`"));
    }

    #[test]
    fn nested_output_root_is_selected_independently_of_grant_order() {
        let directory = TestDirectory::new("nested-grant");
        let source = directory.0.join("source");
        let output = source.join("build");
        std::fs::create_dir_all(&output).unwrap();
        let artifact = output.join("artifact.bin");

        let grants = canonical_grants(crate::FsGrants {
            read_roots: vec![test_grant_root(1, source)],
            write_roots: vec![test_grant_root(2, output)],
        })
        .unwrap();
        let resolved_artifact = resolve_for_check(&artifact).unwrap();
        let selected = grants
            .matching_root(&resolved_artifact, false)
            .expect("nested output is readable through the write root");
        assert_eq!(selected.identity.get(), 2);
        assert_eq!(
            canonical_relative_path(resolved_artifact.strip_prefix(&selected.path).unwrap())
                .unwrap(),
            b"artifact.bin"
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_path_bytes_are_not_lossily_rewritten() {
        use std::os::unix::ffi::OsStrExt;

        let bytes = b"raw-\xff-name";
        let path = real_path(bytes).expect("unix paths preserve arbitrary bytes");
        assert_eq!(path.as_os_str().as_bytes(), bytes);
        assert!(
            canonical_relative_path(&path).is_none(),
            "rooted evidence must reject a path it cannot encode losslessly"
        );
    }

    #[test]
    fn source_reads_and_the_session_root_bypass_sponsorship() {
        let root = Path::new("/staging/session");
        assert!(read_only_open_bypasses_sponsor(
            Path::new("/sources/package/input.txt"),
            root,
            false,
            false,
            false,
        ));
        assert!(read_only_open_bypasses_sponsor(
            root, root, false, false, false,
        ));
        assert!(!read_only_open_bypasses_sponsor(
            Path::new("/staging/session/output.txt"),
            root,
            false,
            false,
            false,
        ));
        assert!(!read_only_open_bypasses_sponsor(
            Path::new("/sources/package/input.txt"),
            root,
            false,
            false,
            true,
        ));
    }

    #[test]
    fn ordinary_namespace_preconditions_are_deferred_to_the_host() {
        let directory = TestDirectory::new("host-precondition");
        let sponsor = crate::FilesystemSponsor::new(&directory.0).unwrap();
        let child = sponsor
            .bind_path(directory.0.join("missing/child"))
            .unwrap();
        assert!(matches!(
            sponsor_preparation(sponsor.prepare_create_directory(&child)),
            Ok(SponsorPreparation::ExpectedHostFailure)
        ));
    }

    #[test]
    fn real_fs_drop_closes_descriptors_and_preserves_named_charges() {
        let directory = TestDirectory::new("drop-close");
        let path = directory.0.join("output.bin");
        std::fs::write(&path, b"1234567").unwrap();
        let sponsor = crate::FilesystemSponsor::new(&directory.0).unwrap();
        let sponsored_path = sponsor.bind_path(&path).unwrap();
        let descriptor = sponsor
            .prepare_create_object_open(&sponsored_path, 7)
            .unwrap()
            .commit()
            .unwrap();
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let mut filesystem = RealFs::new(None, Some(sponsor.clone())).unwrap();
        filesystem.insert(file, path, Some(descriptor), false);
        assert_eq!(sponsor.snapshot().unwrap().open_descriptors, 1);

        drop(filesystem);

        let snapshot = sponsor.snapshot().unwrap();
        assert_eq!(snapshot.open_descriptors, 0);
        assert_eq!(snapshot.entries, 1);
        assert_eq!(snapshot.unique_objects, 1);
        assert_eq!(snapshot.total_logical_bytes, 7);
    }

    #[test]
    fn resource_halt_teardown_closes_remaining_descriptors() {
        let directory = TestDirectory::new("resource-close");
        let path = directory.0.join("output.bin");
        std::fs::write(&path, []).unwrap();
        let sponsor = crate::FilesystemSponsor::with_limits(
            &directory.0,
            crate::FilesystemSponsorLimits {
                maximum_entries: 1,
                maximum_total_logical_bytes: 0,
                maximum_object_extent: 0,
            },
        )
        .unwrap();
        let sponsored_path = sponsor.bind_path(&path).unwrap();
        let descriptor = sponsor
            .prepare_create_object_open(&sponsored_path, 0)
            .unwrap()
            .commit()
            .unwrap();
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let mut filesystem = RealFs::new(None, Some(sponsor.clone())).unwrap();
        filesystem.insert(file, path, Some(descriptor), false);

        assert!(matches!(
            sponsor_preparation(sponsor.prepare_write(&descriptor, 0, 1)),
            Err(super::super::Halt::Resource(_))
        ));
        drop(filesystem);

        let snapshot = sponsor.snapshot().unwrap();
        assert_eq!(snapshot.open_descriptors, 0);
        assert_eq!(snapshot.entries, 1);
        assert_eq!(snapshot.total_logical_bytes, 0);
    }
}
