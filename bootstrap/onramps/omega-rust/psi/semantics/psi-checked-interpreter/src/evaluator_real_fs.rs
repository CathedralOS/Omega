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
//! CANONICALIZED for the check (a not-yet-existing leaf rides its
//! canonicalized parent), so `..` traversal and symlinks that escape a
//! granted tree resolve to their real target and are refused, not
//! string-matched. Fd-based ops need no re-check: an fd only enters the
//! table through an authorized open.
//!
//! Portable by construction: real files ride `std::fs::File` behind the same
//! synthetic-fd table shape the virtual fs uses (no libc, no raw handles), so
//! the provider works wherever the compiler runs. Both providers exhaustively
//! match the same closed operation set. FULL OP PARITY as of 2026-07-10m: every
//! op the virtual fs serves, the real provider serves too (unix-gated where
//! std requires it: symlink/permissions/chown; ENOTSUP on other hosts) --
//! so a build program tested hermetically cannot hit a refusal surprise in
//! real mode on the same host family.

use super::{EvalResult, ExpressionHandle, FilesystemHostOperation, Frame, Value, host_open_flags};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
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
const ENOENT: i32 = 2;
const ENOTDIR: i32 = 20;

/// Canonicalized [`crate::FsGrants`]: the roots a scoped run may read/write
/// under, resolved once at construction so prefix checks compare real paths.
struct Grants {
    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
}

/// Canonicalize a root at grant-construction time. A root that does not
/// resolve (e.g. a build dir created later in the run) keeps its spelled
/// path -- ops under it then authorize via their own canonicalized parents,
/// which fail ENOENT until the root exists, exactly like the real OS.
fn canonical_root(root: &Path) -> PathBuf {
    root.canonicalize().unwrap_or_else(|_| root.to_path_buf())
}

impl Grants {
    fn allows(&self, resolved: &Path, write: bool) -> bool {
        let in_write = self
            .write_roots
            .iter()
            .any(|root| resolved.starts_with(root));
        if write {
            return in_write;
        }
        // A write root implicitly grants read-back (stage-then-verify is the
        // normal build shape); read_roots are the read-ONLY trees.
        in_write
            || self
                .read_roots
                .iter()
                .any(|root| resolved.starts_with(root))
    }
}

/// One real open descriptor: the file handle plus the RESOLVED path it was
/// opened at (kept for the ops std serves path-wise, e.g. `read_dir` -- a
/// directory listing needs the path back, since std has no fd-based dirent
/// read).
struct RealFd {
    file: std::fs::File,
    path: PathBuf,
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
}

impl RealFs {
    pub(super) fn new(grants: Option<crate::FsGrants>) -> Self {
        Self {
            files: BTreeMap::new(),
            next_fd: 3,
            errno: 0,
            grants: grants.map(|grants| Grants {
                read_roots: grants
                    .read_roots
                    .iter()
                    .map(|r| canonical_root(r))
                    .collect(),
                write_roots: grants
                    .write_roots
                    .iter()
                    .map(|r| canonical_root(r))
                    .collect(),
            }),
        }
    }

    pub(super) fn is_scoped(&self) -> bool {
        self.grants.is_some()
    }

    fn insert(&mut self, file: std::fs::File, path: PathBuf) -> i64 {
        let fd = self.next_fd;
        self.next_fd += 1;
        self.files.insert(fd, RealFd { file, path });
        i64::from(fd)
    }
}

fn real_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
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
        children.push((
            dir_entry
                .file_name()
                .to_string_lossy()
                .into_owned()
                .into_bytes(),
            d_type,
        ));
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
    file.seek(SeekFrom::Start(offset.max(0) as u64))?;
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
    file.seek(SeekFrom::Start(offset.max(0) as u64))?;
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

impl<'program> super::Evaluator<'program> {
    /// Mirror of `try_filesystem_call` against the REAL filesystem. The match
    /// exhaustively covers the same closed operation type as the virtual
    /// provider, so neither provider can silently omit a canonical operation.
    pub(super) fn try_real_filesystem_call(
        &mut self,
        operation: FilesystemHostOperation,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<Value> {
        // Two-phase per op: resolve arguments via the evaluator FIRST (that
        // borrow of self ends), then touch self.real_fs.
        let result: i64 = match operation {
            FilesystemHostOperation::Create => {
                // O_WRONLY|O_CREAT|O_TRUNC: create/truncate, writable.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                match self.authorized_path(&path, true) {
                    Some(path) => {
                        let opened = std::fs::OpenOptions::new()
                            .write(true)
                            .create(true)
                            .truncate(true)
                            .open(&path);
                        self.real_result_fd(opened, path)
                    }
                    None => -1,
                }
            }
            FilesystemHostOperation::Open | FilesystemHostOperation::OpenCreate => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let flags = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                // `open_create`'s trailing creation mode; `open` has no third
                // argument, so this reads ZII 0 there and is never applied
                // (O_CREAT is what makes a mode meaningful).
                let mode = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as u32;
                // Flag bits decode via the same host-flag mirror the virtual
                // fs uses (host_open_flags -- the program was compiled for
                // `host()`, so the numerology matches).
                let access = flags & 0x3;
                let wants_write = access == 1
                    || access == 2
                    || host_open_flags::o_creat(flags)
                    || host_open_flags::o_trunc(flags)
                    || host_open_flags::o_append(flags);
                match self.authorized_path(&path, wants_write) {
                    Some(path) => {
                        let options = open_options_for(
                            flags,
                            mode,
                            operation == FilesystemHostOperation::OpenCreate,
                        );
                        self.real_result_fd(open_real(&options, &path, wants_write), path)
                    }
                    None => -1,
                }
            }
            FilesystemHostOperation::OpenPathHandle => {
                // Real-mode model of CreateFileA's metadata/query use. The
                // shared helper adds FILE_FLAG_BACKUP_SEMANTICS for a directory
                // on Windows, so the same synthetic handle table serves files
                // and directories.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                match self.authorized_path(&path, false) {
                    Some(path) => {
                        let mut options = std::fs::OpenOptions::new();
                        options.read(true);
                        match open_real(&options, &path, false) {
                            Ok(file) => self.real_fs_mut().insert(file, path),
                            Err(error) => {
                                self.real_fs_mut().errno = win32_error_code(&error);
                                -1
                            }
                        }
                    }
                    None => {
                        let real = self.real_fs_mut();
                        real.errno = if real.errno == ENOENT { 2 } else { 5 };
                        -1
                    }
                }
            }
            FilesystemHostOperation::Read => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let count = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                let outcome = {
                    let real = self.real_fs_mut();
                    match real.files.get_mut(&fd) {
                        Some(entry) => {
                            let mut buffer = vec![0u8; count];
                            match entry.file.read(&mut buffer) {
                                Ok(n) => {
                                    buffer.truncate(n);
                                    Ok(buffer)
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
                        self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
                        n
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            FilesystemHostOperation::Write => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let bytes = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                let real = self.real_fs_mut();
                match real.files.get_mut(&fd) {
                    Some(entry) => match entry.file.write(&bytes) {
                        Ok(n) => n as i64,
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
            FilesystemHostOperation::Seek => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let offset = self.eval_fs_scalar(arguments.get(1).copied(), frame)?;
                let whence = self.eval_fs_scalar(arguments.get(2).copied(), frame)?;
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
            FilesystemHostOperation::Close => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let real = self.real_fs_mut();
                if real.files.remove(&fd).is_some() {
                    0 // the File drop closes the real descriptor
                } else {
                    real.errno = EBADF;
                    -1
                }
            }
            FilesystemHostOperation::CloseHandle => {
                let handle = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let real = self.real_fs_mut();
                if real.files.remove(&handle).is_some() {
                    1 // Win32 BOOL success; dropping File closes the handle.
                } else {
                    real.errno = 6; // ERROR_INVALID_HANDLE
                    0
                }
            }
            FilesystemHostOperation::Duplicate => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let real = self.real_fs_mut();
                let cloned = match real.files.get(&fd) {
                    Some(entry) => entry
                        .file
                        .try_clone()
                        .map(|file| (file, entry.path.clone()))
                        .map_err(|error| io_errno(&error)),
                    None => Err(EBADF),
                };
                match cloned {
                    Ok((file, path)) => real.insert(file, path),
                    Err(errno) => {
                        real.errno = errno;
                        -1
                    }
                }
            }
            FilesystemHostOperation::SetLen => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let length = self.eval_fs_scalar(arguments.get(1).copied(), frame)?;
                let real = self.real_fs_mut();
                match real.files.get_mut(&fd) {
                    Some(entry) => match entry.file.set_len(length.max(0) as u64) {
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
            FilesystemHostOperation::Sync | FilesystemHostOperation::SyncData => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
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
            FilesystemHostOperation::Remove | FilesystemHostOperation::RemoveName => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                match self.authorized_path(&path, true) {
                    Some(path) => self.real_result_unit(std::fs::remove_file(path)),
                    None => -1,
                }
            }
            FilesystemHostOperation::CreateDir | FilesystemHostOperation::CreateDirName => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                match self.authorized_path(&path, true) {
                    Some(path) => self.real_result_unit(std::fs::create_dir(path)),
                    None => -1,
                }
            }
            FilesystemHostOperation::RemoveDir | FilesystemHostOperation::RemoveDirName => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                match self.authorized_path(&path, true) {
                    Some(path) => self.real_result_unit(std::fs::remove_dir(path)),
                    None => -1,
                }
            }
            FilesystemHostOperation::Rename => {
                let from = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let to = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                // BOTH ends need write authority: a rename removes `from` and
                // creates `to`.
                match (
                    self.authorized_path(&from, true),
                    self.authorized_path(&to, true),
                ) {
                    (Some(from), Some(to)) => self.real_result_unit(std::fs::rename(from, to)),
                    _ => -1,
                }
            }
            FilesystemHostOperation::ReadAt => {
                // `pread(fd, buf, count, offset)`: read at an absolute offset
                // WITHOUT moving the cursor. Emulated portably (std has no
                // cross-platform pread): seek, read, restore.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let count = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                let offset = self.eval_fs_scalar(arguments.get(3).copied(), frame)?;
                let outcome = {
                    let real = self.real_fs_mut();
                    match real.files.get_mut(&fd) {
                        Some(entry) => positioned_read(&mut entry.file, offset, count)
                            .map_err(|error| io_errno(&error)),
                        None => Err(EBADF),
                    }
                };
                match outcome {
                    Ok(bytes) => {
                        let n = bytes.len() as i64;
                        self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
                        n
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            FilesystemHostOperation::WriteAt => {
                // `pwrite(fd, buf, offset)`: write at an absolute offset
                // WITHOUT moving the cursor (same emulation).
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let bytes = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                let offset = self.eval_fs_scalar(arguments.get(2).copied(), frame)?;
                let real = self.real_fs_mut();
                match real.files.get_mut(&fd) {
                    Some(entry) => match positioned_write(&mut entry.file, offset, &bytes) {
                        Ok(n) => n as i64,
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
            FilesystemHostOperation::ReadDir => {
                // `read_dir(fd, buf, count, &position)` -- the virtual
                // dispatcher's contract, mirrored. Pack `.`/`..` plus immediate
                // children as Darwin dirent records and return the next window
                // of complete records. The synthetic byte cursor lets repeated
                // calls drain directories larger than one caller buffer. Names
                // come from `std::fs::read_dir` and are sorted for determinism;
                // native getdirentries order remains filesystem-defined.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let count = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                let position = self.read_fs_position(arguments.get(3).copied(), frame);
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
                        let start = position.max(0) as usize;
                        let (chunk, next_position) =
                            super::dirent_record_chunk(&records, start, count);
                        if chunk.is_empty() {
                            0
                        } else {
                            let n = chunk.len();
                            self.write_fs_buffer(arguments.get(1).copied(), frame, chunk);
                            self.write_fs_position(
                                arguments.get(3).copied(),
                                frame,
                                next_position as i64,
                            );
                            n as i64
                        }
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            FilesystemHostOperation::FindFirst => {
                // `find_first(pattern, &data)` -- the windows dir-walk seam
                // (fs rung 3a) served against the real filesystem: strip the
                // `/*` tail (the impl joins with `/`, which Win32 accepts),
                // list the directory (the same dot-prefixed sorted set
                // read_dir packs), snapshot the tail into a cursor keyed by a
                // fresh handle, and fill the FIRST entry's find-data record.
                let pattern = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let listed = match pattern.strip_suffix(b"/*") {
                    Some(dir_path) => match self.authorized_path(dir_path, false) {
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
                        self.write_find_data(arguments.get(1).copied(), frame, &name, is_dir);
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
            FilesystemHostOperation::FindNext => {
                // Cursor-only (the snapshot was taken at find_first) -- the
                // same arm shape as the hermetic dispatcher.
                let handle = self.eval_fs_scalar(arguments.first().copied(), frame)?;
                match self
                    .virtual_finds
                    .get_mut(&handle)
                    .and_then(std::collections::VecDeque::pop_front)
                {
                    Some((name, is_dir)) => {
                        self.write_find_data(arguments.get(1).copied(), frame, &name, is_dir);
                        1
                    }
                    None => 0,
                }
            }
            FilesystemHostOperation::FindClose => {
                let handle = self.eval_fs_scalar(arguments.first().copied(), frame)?;
                if self.virtual_finds.remove(&handle).is_some() {
                    1
                } else {
                    0
                }
            }
            FilesystemHostOperation::ReadMetadata
            | FilesystemHostOperation::ReadSymlinkMetadata => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let authorized = if operation == FilesystemHostOperation::ReadSymlinkMetadata {
                    self.authorized_path_no_follow(&path, false)
                } else {
                    self.authorized_path(&path, false)
                };
                let looked_up = match authorized {
                    Some(path) => {
                        if operation == FilesystemHostOperation::ReadMetadata {
                            std::fs::metadata(path)
                        } else {
                            std::fs::symlink_metadata(path)
                        }
                    }
                    None => {
                        return Ok(Value::Int(-1));
                    }
                };
                match looked_up {
                    Ok(metadata) => {
                        self.write_real_fs_stat(arguments.get(1).copied(), frame, &metadata);
                        0
                    }
                    Err(error) => {
                        self.real_fs_mut().errno = io_errno(&error);
                        -1
                    }
                }
            }
            FilesystemHostOperation::ReadFileMetadata => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let looked_up = match self.real_fs_mut().files.get(&fd) {
                    Some(entry) => entry.file.metadata().map_err(|error| io_errno(&error)),
                    None => Err(EBADF),
                };
                match looked_up {
                    Ok(metadata) => {
                        self.write_real_fs_stat(arguments.get(1).copied(), frame, &metadata);
                        0
                    }
                    Err(errno) => {
                        self.real_fs_mut().errno = errno;
                        -1
                    }
                }
            }
            FilesystemHostOperation::Errno => i64::from(self.real_fs_mut().errno),
            FilesystemHostOperation::Canonicalize => {
                // `realpath(path, buf)`: NUL-terminated resolved path into the
                // buffer; non-zero success flag, 0 (NULL) + errno on failure --
                // the virtual contract's shape.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                match self.authorized_path(&path, false) {
                    Some(path) => match std::fs::canonicalize(&path) {
                        Ok(resolved) => {
                            let mut bytes = resolved.to_string_lossy().into_owned().into_bytes();
                            bytes.push(0);
                            self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
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
            FilesystemHostOperation::HardLink => {
                // `link(original, link)`: real inodes, unlike the virtual
                // byte-copy approximation. Read authority on the original,
                // write authority on the new name.
                let original = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let link = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                match (
                    self.authorized_path(&original, false),
                    self.authorized_path(&link, true),
                ) {
                    (Some(original), Some(link)) => {
                        self.real_result_unit(std::fs::hard_link(original, link))
                    }
                    _ => -1,
                }
            }
            FilesystemHostOperation::CreateHardLink => {
                // `CreateHardLinkA(link, existing, security)` -- the windows
                // primitive's arg order (NEW link first) and BOOL result
                // (1 success / 0 failure). Served portably via std like
                // `hard_link` above; errno doubles as this provider's modeled
                // GetLastError slot and therefore stores Win32 codes here.
                let link = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let existing = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                match (
                    self.authorized_path(&existing, false),
                    self.authorized_path(&link, true),
                ) {
                    (Some(existing), Some(link)) => match std::fs::hard_link(existing, link) {
                        Ok(()) => 1,
                        Err(error) => {
                            self.real_fs_mut().errno = win32_error_code(&error);
                            0
                        }
                    },
                    _ => 0,
                }
            }
            FilesystemHostOperation::GetOsfHandle => {
                // The fd -> HANDLE bridge (session slice 4a). The real
                // provider's files ride std::fs behind SYNTHETIC fds by
                // design (no raw handles), so its handles are the fds
                // themselves -- identity, like the hermetic model; -2 for an
                // unknown fd (msvcrt's bad-fd spelling).
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                if self.real_fs_mut().files.contains_key(&fd) {
                    i64::from(fd)
                } else {
                    -2
                }
            }
            FilesystemHostOperation::FinalPathNameByHandle => {
                // Resolve an open handle (= synthetic fd) to its final path:
                // std::fs::canonicalize of the entry's stored path (on a
                // windows host that IS the \\?\-prefixed final path, matching
                // native GetFinalPathNameByHandleA). Win32 return contract:
                // length without the NUL when it fits, required size with the
                // NUL when too small, 0 on failure; errno is this provider's
                // modeled GetLastError slot.
                let handle = self.eval_fs_scalar(arguments.first().copied(), frame)?;
                let capacity = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                let path = self
                    .real_fs_mut()
                    .files
                    .get(&(handle as i32))
                    .map(|entry| entry.path.clone());
                match path {
                    Some(path) => match std::fs::canonicalize(path) {
                        Ok(path) => {
                            let path = path.display().to_string().into_bytes();
                            if path.len() + 1 <= capacity {
                                let mut bytes = path.clone();
                                bytes.push(0);
                                self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
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
            FilesystemHostOperation::SetFileTime => {
                // `SetFileTime(handle, creation, access_ft, write_ft)` (session
                // slice 4b): apply the WRITE time from its FILETIME buffer via
                // std's set_modified, like `set_file_times` above. BOOL result;
                // 0 for a bad handle or a failed stamp; errno models
                // GetLastError for the wrapper's immediate capture.
                let handle = self.eval_fs_scalar(arguments.first().copied(), frame)?;
                let write_ft = self.eval_fs_bytes(arguments.get(3).copied(), frame)?;
                let filetime = write_ft
                    .get(0..8)
                    .and_then(|s| <[u8; 8]>::try_from(s).ok())
                    .map(i64::from_le_bytes)
                    .unwrap_or(0);
                let secs = filetime / 10_000_000 - 11_644_473_600;
                let real = self.real_fs_mut();
                match real.files.get_mut(&(handle as i32)) {
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
            FilesystemHostOperation::Symlink => {
                // `symlink(target, linkpath)`: the TARGET is stored verbatim
                // (never dereferenced here), so only the link path needs write
                // authority. Unix-only in std; elsewhere ENOTSUP.
                let target = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let link = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                match self.authorized_path(&link, true) {
                    Some(link) => {
                        #[cfg(unix)]
                        {
                            self.real_result_unit(std::os::unix::fs::symlink(
                                real_path(&target),
                                link,
                            ))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (target, link);
                            self.real_fs_mut().errno = ENOTSUP;
                            -1
                        }
                    }
                    None => -1,
                }
            }
            FilesystemHostOperation::ReadLink => {
                // `readlink(path, buf, count)`: target bytes into the buffer,
                // returns the count written.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let count = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                match self.authorized_path_no_follow(&path, false) {
                    Some(path) => match std::fs::read_link(&path) {
                        Ok(target) => {
                            let bytes = target.to_string_lossy().into_owned().into_bytes();
                            let n = bytes.len().min(count);
                            self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes[..n]);
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
            FilesystemHostOperation::SetPermissions => {
                // `chmod(path, mode)`: metadata mutation = write authority.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let mode = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as u32;
                match self.authorized_path(&path, true) {
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
            FilesystemHostOperation::SetFilePermissions => {
                // `fchmod(fd, mode)`: by descriptor; no re-authorization (the
                // fd entered through an authorized open).
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let mode = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as u32;
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
            FilesystemHostOperation::SetFileTimes => {
                // `futimens(fd, times)`: two packed timespecs (atime, mtime);
                // the model (virtual and real alike) applies the MODIFIED time
                // -- times[1].tv_sec at byte offset 16.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let times = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                let mtime_secs = times
                    .get(16..24)
                    .map(|bytes| i64::from_le_bytes(bytes.try_into().unwrap()))
                    .unwrap_or(0);
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
            FilesystemHostOperation::LockFile => {
                // `flock(fd, op)`: LOCK_SH=1 LOCK_EX=2 LOCK_NB=4 LOCK_UN=8,
                // served by std's advisory file locks on the real handle.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let operation = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let real = self.real_fs_mut();
                match real.files.get(&fd) {
                    Some(entry) => real_lock(&entry.file, operation, &mut real.errno),
                    None => {
                        real.errno = EBADF;
                        -1
                    }
                }
            }
            FilesystemHostOperation::LockFileEx => {
                // Win32 LockFileEx semantics over the provider's synthetic
                // handle. The exact byte range is intentionally ignored here:
                // the std wrapper always supplies offset zero + u64::MAX.
                let fd = self.eval_fs_scalar(arguments.first().copied(), frame)? as i32;
                let flags = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let real = self.real_fs_mut();
                match real.files.get(&fd) {
                    Some(entry) => real_lock_win32(&entry.file, flags, &mut real.errno),
                    None => {
                        real.errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                }
            }
            FilesystemHostOperation::UnlockFile => {
                let fd = self.eval_fs_scalar(arguments.first().copied(), frame)? as i32;
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
            FilesystemHostOperation::GetLastError => i64::from(self.real_fs_mut().errno),
            FilesystemHostOperation::ChangeOwner | FilesystemHostOperation::ChangeOwnerNoFollow => {
                // `chown`/`lchown(path, uid, gid)`: -1 leaves the component
                // alone (None). Metadata mutation = write authority.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let uid = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let gid = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                let authorized = if operation == FilesystemHostOperation::ChangeOwnerNoFollow {
                    self.authorized_path_no_follow(&path, true)
                } else {
                    self.authorized_path(&path, true)
                };
                match authorized {
                    Some(path) => {
                        #[cfg(unix)]
                        {
                            let owner = (uid >= 0).then_some(uid as u32);
                            let group = (gid >= 0).then_some(gid as u32);
                            let outcome =
                                if operation == FilesystemHostOperation::ChangeOwnerNoFollow {
                                    std::os::unix::fs::lchown(path, owner, group)
                                } else {
                                    std::os::unix::fs::chown(path, owner, group)
                                };
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
            FilesystemHostOperation::ChangeFileOwner => {
                // `fchown(fd, uid, gid)`: by descriptor.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let uid = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let gid = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
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
            FilesystemHostOperation::UnlinkAt => {
                // `unlinkat(dirfd, name, flags)`: resolve against the dirfd's
                // OPENED path (the same trick read_dir rides -- std has no fd
                // relative ops); flags & AT_REMOVEDIR(0x80) removes a dir.
                let dirfd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let name = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                let flags = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                let joined = match self.real_fs_mut().files.get(&dirfd) {
                    Some(entry) => entry.path.join(real_path(&name)),
                    None => {
                        self.real_fs_mut().errno = EBADF;
                        return Ok(Value::Int(-1));
                    }
                };
                let joined_bytes = joined.to_string_lossy().into_owned().into_bytes();
                match self.authorized_path(&joined_bytes, true) {
                    Some(path) => {
                        if flags & 0x80 != 0 {
                            self.real_result_unit(std::fs::remove_dir(path))
                        } else {
                            self.real_result_unit(std::fs::remove_file(path))
                        }
                    }
                    None => -1,
                }
            }
            FilesystemHostOperation::OpenAt => {
                // `openat(dirfd, name, flags)`: join against the dirfd's opened
                // path, then the ordinary open (same flag decode + grants).
                let dirfd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let name = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                let flags = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                let joined = match self.real_fs_mut().files.get(&dirfd) {
                    Some(entry) => entry.path.join(real_path(&name)),
                    None => {
                        self.real_fs_mut().errno = EBADF;
                        return Ok(Value::Int(-1));
                    }
                };
                let joined_bytes = joined.to_string_lossy().into_owned().into_bytes();
                let access = flags & 0x3;
                let wants_write = access == 1
                    || access == 2
                    || host_open_flags::o_creat(flags)
                    || host_open_flags::o_trunc(flags)
                    || host_open_flags::o_append(flags);
                match self.authorized_path(&joined_bytes, wants_write) {
                    Some(path) => {
                        let options = open_options_for(flags, 0, false);
                        self.real_result_fd(open_real(&options, &path, wants_write), path)
                    }
                    None => -1,
                }
            }
        };
        Ok(Value::Int(result))
    }

    fn real_fs_mut(&mut self) -> &mut RealFs {
        self.real_fs
            .as_mut()
            .expect("try_real_filesystem_call only runs in real mode")
    }

    /// Authorize a path-taking op against the grants (no-op when unscoped).
    /// `None` means REFUSED with errno already set: EACCES outside the
    /// granted roots, ENOENT when the path's parent does not even resolve.
    fn authorized_path(&mut self, path_bytes: &[u8], write: bool) -> Option<PathBuf> {
        self.authorized_path_with_follow(path_bytes, write, true)
    }

    /// The NO-FOLLOW variant for symlink-INSPECTING ops (read_link,
    /// read_symlink_metadata, lchown): full canonicalization would resolve
    /// the final symlink and the op would run on its TARGET (read_link on
    /// the target is EINVAL; lstat would report the wrong file -- probed
    /// 2026-07-10m). Resolution goes parent-canonical + reattached leaf, so
    /// the grant check still compares real locations while the leaf link
    /// itself stays the operand.
    fn authorized_path_no_follow(&mut self, path_bytes: &[u8], write: bool) -> Option<PathBuf> {
        self.authorized_path_with_follow(path_bytes, write, false)
    }

    fn authorized_path_with_follow(
        &mut self,
        path_bytes: &[u8],
        write: bool,
        follow: bool,
    ) -> Option<PathBuf> {
        let path = real_path(path_bytes);
        let real = self.real_fs_mut();
        let Some(grants) = &real.grants else {
            return Some(path); // unscoped: full process authority
        };
        let resolved = if follow {
            resolve_for_check(&path)
        } else {
            resolve_parent_for_check(&path)
        };
        let Some(resolved) = resolved else {
            real.errno = ENOENT;
            return None;
        };
        if grants.allows(&resolved, write) {
            // Operate on the RESOLVED path: the authorized location and the
            // operated-on location must be the same real file.
            Some(resolved)
        } else {
            real.errno = EACCES;
            None
        }
    }

    fn real_result_fd(&mut self, opened: std::io::Result<std::fs::File>, path: PathBuf) -> i64 {
        match opened {
            Ok(file) => self.real_fs_mut().insert(file, path),
            Err(error) => {
                self.real_fs_mut().errno = io_errno(&error);
                -1
            }
        }
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
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
        metadata: &std::fs::Metadata,
    ) {
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
        self.write_fs_stat(argument, frame, mode, size, mtime_secs);
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
