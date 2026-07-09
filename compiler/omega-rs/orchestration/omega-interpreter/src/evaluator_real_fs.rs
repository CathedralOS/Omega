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
//! the provider works wherever the compiler runs. The op-name set mirrors the
//! virtual dispatcher one-for-one; ops outside the core subset report
//! ENOTSUP and -1 (loud, never silently wrong) until a later slice.

use super::{EvalResult, ExpressionHandle, Frame, Value, host_open_flags};
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
    /// Mirror of `try_filesystem_call` against the REAL filesystem. Same
    /// contract: `Ok(None)` when `method` is not a filesystem op. The match
    /// arms cover the virtual dispatcher's op names ONE-FOR-ONE so a
    /// filesystem op can never fall through to a non-fs resolution path in
    /// real mode.
    pub(super) fn try_real_filesystem_call(
        &mut self,
        method: &str,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        // Two-phase per op: resolve arguments via the evaluator FIRST (that
        // borrow of self ends), then touch self.real_fs.
        let result: i64 = match method {
            "create" => {
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
            "open" | "open_create" => {
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
                        if host_open_flags::o_creat(flags) && method == "open_create" {
                            use std::os::unix::fs::OpenOptionsExt;
                            options.mode(mode & 0o7777);
                        }
                        #[cfg(not(unix))]
                        let _ = mode; // windows: creation mode has no direct analogue
                        self.real_result_fd(options.open(&path), path)
                    }
                    None => -1,
                }
            }
            "read" => {
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
            "write" => {
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
            "seek" => {
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
            "close" => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let real = self.real_fs_mut();
                if real.files.remove(&fd).is_some() {
                    0 // the File drop closes the real descriptor
                } else {
                    real.errno = EBADF;
                    -1
                }
            }
            "duplicate" => {
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
            "set_len" => {
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
            "sync" | "sync_data" => {
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
            "remove" => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                match self.authorized_path(&path, true) {
                    Some(path) => self.real_result_unit(std::fs::remove_file(path)),
                    None => -1,
                }
            }
            "create_dir" | "create_dir_name" => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                match self.authorized_path(&path, true) {
                    Some(path) => self.real_result_unit(std::fs::create_dir(path)),
                    None => -1,
                }
            }
            "remove_dir" => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                match self.authorized_path(&path, true) {
                    Some(path) => self.real_result_unit(std::fs::remove_dir(path)),
                    None => -1,
                }
            }
            "rename" => {
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
            "read_at" => {
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
            "write_at" => {
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
            "read_dir" => {
                // `read_dir(fd, buf, count, &position)` -- the virtual
                // dispatcher's contract, mirrored: the first call (position
                // == 0) packs `.`/`..` + the immediate children as darwin
                // dirent records and sets `position`; later calls return 0
                // (end). Names come from a real `std::fs::read_dir` of the
                // fd's opened path (std has no fd-based dirent read), sorted
                // for determinism -- native getdirentries order is
                // filesystem-defined anyway, so no program may rely on it.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let count = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                let position = self.read_fs_position(arguments.get(3).copied(), frame);
                if position != 0 {
                    0
                } else {
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
                            let n = records.len().min(count);
                            self.write_fs_buffer(arguments.get(1).copied(), frame, &records[..n]);
                            // Any non-zero marker so the next call reports end.
                            self.write_fs_position(
                                arguments.get(3).copied(),
                                frame,
                                n.max(1) as i64,
                            );
                            n as i64
                        }
                        Err(errno) => {
                            self.real_fs_mut().errno = errno;
                            -1
                        }
                    }
                }
            }
            "read_metadata" | "read_symlink_metadata" => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let looked_up = match self.authorized_path(&path, false) {
                    Some(path) => {
                        if method == "read_metadata" {
                            std::fs::metadata(path)
                        } else {
                            std::fs::symlink_metadata(path)
                        }
                    }
                    None => {
                        return Ok(Some(Value::Int(-1)));
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
            "read_file_metadata" => {
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
            "errno" => i64::from(self.real_fs_mut().errno),
            // NEXT-SLICE ops (the *at family, locks, ownership, permissions,
            // times, links, canonicalize): loud not-supported, never silently
            // wrong. Listed by name so the fall-through `_` stays "not a
            // filesystem op".
            "open_at"
            | "unlink_at"
            | "lock_file"
            | "set_file_permissions"
            | "set_permissions"
            | "set_file_times"
            | "change_owner"
            | "change_owner_no_follow"
            | "change_file_owner"
            | "hard_link"
            | "symlink"
            | "read_link"
            | "canonicalize" => {
                self.real_fs_mut().errno = ENOTSUP;
                -1
            }
            _ => return Ok(None),
        };
        Ok(Some(Value::Int(result)))
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
        let path = real_path(path_bytes);
        let real = self.real_fs_mut();
        let Some(grants) = &real.grants else {
            return Some(path); // unscoped: full process authority
        };
        let Some(resolved) = resolve_for_check(&path) else {
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
