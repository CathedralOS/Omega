//! REAL-filesystem provider for the interpreter (build.omg rung 1, TASKS_FS
//! open-work #3): `build.omg` runs INTERPRETED with a real `Filesystem`
//! capability so it can copy assets itself. Strictly OPT-IN
//! (`FilesystemAccess::RealUnscoped`); the default hermetic virtual fs -- the
//! differential oracle -- is untouched. Grant/scope plumbing (read: source
//! tree; write: build dir) is the NEXT rung; the type name telegraphs that
//! this mode is unscoped.
//!
//! Portable by construction: real files ride `std::fs::File` behind the same
//! synthetic-fd table shape the virtual fs uses (no libc, no raw handles), so
//! the provider works wherever the compiler runs. The op-name set mirrors the
//! virtual dispatcher one-for-one; ops outside rung 1's core subset report
//! ENOTSUP and -1 (loud, never silently wrong) until a later slice.

use super::{EvalResult, ExpressionHandle, Frame, Value, host_open_flags};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};

/// Errno for real-mode failures, from the host `io::Error` when available.
fn io_errno(error: &std::io::Error) -> i32 {
    error.raw_os_error().unwrap_or(5) // EIO when the host gives no code
}

/// ENOTSUP differs per OS (macOS 45, linux 95, windows maps EOPNOTSUPP=130);
/// the wrapper only tests `rc < 0` + errno passthrough, so macOS's value is
/// fine as the single modeled "this provider slice does not do that" code.
const ENOTSUP: i32 = 45;
const EBADF: i32 = 9;

pub(super) struct RealFs {
    /// Synthetic fd -> real open file. Same table shape as `virtual_fds`;
    /// descriptors start at 3 (0/1/2 are the standard streams).
    files: BTreeMap<i32, std::fs::File>,
    next_fd: i32,
    /// Thread-local errno model, mirroring `virtual_errno`: set from the host
    /// `io::Error` on a failing op, read back by `errno`.
    pub(super) errno: i32,
}

impl RealFs {
    pub(super) fn new() -> Self {
        Self {
            files: BTreeMap::new(),
            next_fd: 3,
            errno: 0,
        }
    }

    fn insert(&mut self, file: std::fs::File) -> i64 {
        let fd = self.next_fd;
        self.next_fd += 1;
        self.files.insert(fd, file);
        i64::from(fd)
    }
}

fn real_path(bytes: &[u8]) -> std::path::PathBuf {
    std::path::PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
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
                let opened = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(real_path(&path));
                self.real_result_fd(opened)
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
                self.real_result_fd(options.open(real_path(&path)))
            }
            "read" => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let count = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as usize;
                let outcome = {
                    let real = self.real_fs_mut();
                    match real.files.get_mut(&fd) {
                        Some(file) => {
                            let mut buffer = vec![0u8; count];
                            match file.read(&mut buffer) {
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
                    Some(file) => match file.write(&bytes) {
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
                    Some(file) => match file.seek(position) {
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
                    Some(file) => file.try_clone().map_err(|error| io_errno(&error)),
                    None => Err(EBADF),
                };
                match cloned {
                    Ok(file) => real.insert(file),
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
                    Some(file) => match file.set_len(length.max(0) as u64) {
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
                    Some(file) => match file.sync_all() {
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
                self.real_result_unit(std::fs::remove_file(real_path(&path)))
            }
            "create_dir" | "create_dir_name" => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                self.real_result_unit(std::fs::create_dir(real_path(&path)))
            }
            "remove_dir" => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                self.real_result_unit(std::fs::remove_dir(real_path(&path)))
            }
            "rename" => {
                let from = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let to = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                self.real_result_unit(std::fs::rename(real_path(&from), real_path(&to)))
            }
            "read_metadata" | "read_symlink_metadata" => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let looked_up = if method == "read_metadata" {
                    std::fs::metadata(real_path(&path))
                } else {
                    std::fs::symlink_metadata(real_path(&path))
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
                    Some(file) => file.metadata().map_err(|error| io_errno(&error)),
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
            // NEXT-SLICE ops (the *at family, positioned I/O, locks,
            // ownership, permissions, times, links, dir enumeration,
            // canonicalize): loud not-supported, never silently wrong. Listed
            // by name so the fall-through `_` stays "not a filesystem op".
            "read_at"
            | "write_at"
            | "open_at"
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
            | "canonicalize"
            | "read_dir" => {
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

    fn real_result_fd(&mut self, opened: std::io::Result<std::fs::File>) -> i64 {
        match opened {
            Ok(file) => self.real_fs_mut().insert(file),
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
