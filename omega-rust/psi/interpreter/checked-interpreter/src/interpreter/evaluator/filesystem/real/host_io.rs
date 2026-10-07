use super::{EINVAL, ENOTDIR, io_errno};
use crate::interpreter::evaluator::EvalResult;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub(super) fn real_path(bytes: &[u8]) -> Option<PathBuf> {
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

pub(super) fn real_os_byte_slice(value: &std::ffi::OsStr) -> Option<&[u8]> {
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

pub(super) fn real_os_bytes(value: &std::ffi::OsStr) -> Option<Vec<u8>> {
    real_os_byte_slice(value).map(<[u8]>::to_vec)
}

#[derive(Clone, Copy)]
pub(super) enum DirectoryEntrySnapshotKind {
    PackedRecords,
    FindCursor,
}

/// List a real directory as `(name, d_type)` entries: `.`/`..` first, then
/// immediate children sorted for deterministic interpreter behavior. The
/// selected consumer decides whether to reserve only retained names or also
/// the packed-dirent lane. Host failures remain errno results; compiler-owned
/// snapshot ceilings are evaluator resource halts.
pub(super) fn real_directory_entries(
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
pub(super) fn positioned_read(
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
pub(super) fn positioned_write(
    file: &mut std::fs::File,
    offset: i64,
    bytes: &[u8],
) -> std::io::Result<usize> {
    let saved = file.stream_position()?;
    let offset = u64::try_from(offset.max(0)).expect("nonnegative i64 fits in u64");
    file.seek(SeekFrom::Start(offset))?;
    let outcome = file.write(bytes);
    file.seek(SeekFrom::Start(saved))?;
    outcome
}

/// Open through `options`, serving DIRECTORIES too: a read-only open of a
/// directory (the `open_at`/`unlink_at`/`read_dir` dirfd mint) needs
/// FILE_FLAG_BACKUP_SEMANTICS on windows -- std's plain OpenOptions refuses
/// directory handles there, while unix serves `open(dir, O_RDONLY)` natively.
/// Write-intent opens are NOT redirected, so a write-open of a directory
/// fails on windows exactly like unix's EISDIR.
pub(super) fn open_real(
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
pub(super) fn open_options_for(
    flags: i32,
    mode: u32,
    apply_creation_mode: bool,
) -> std::fs::OpenOptions {
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
pub(super) fn real_lock(file: &std::fs::File, operation: i32, errno: &mut i32) -> i64 {
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
pub(super) fn real_lock_win32(file: &std::fs::File, flags: i32, last_error: &mut i32) -> i64 {
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

pub(super) fn win32_error_code(error: &std::io::Error) -> i32 {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::NotFound => 2,
        ErrorKind::PermissionDenied => 5,
        ErrorKind::AlreadyExists => 183,
        ErrorKind::WouldBlock => 33,
        _ => error.raw_os_error().unwrap_or(1),
    }
}
