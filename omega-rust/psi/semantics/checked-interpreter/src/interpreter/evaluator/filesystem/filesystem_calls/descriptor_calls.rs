//! Operations on an open descriptor or handle: reads, writes, seeks,
//! duplication, extents, syncs, locks, and descriptor-scoped metadata.

use crate::FilesystemMetadataObservationKind;
use crate::interpreter::evaluator::{
    EvalResult, PreparedFilesystemCall, VIRTUAL_MTIME_SECS, VirtualFd, synthetic_handle_fd,
};

impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
    pub(super) fn serve_read(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Read { fd, buffer, count } = call else {
            unreachable!("dispatched serve_read")
        };
        Ok({
            match self.virtual_read_n(fd, count.host) {
                Some(bytes) => {
                    let n = bytes.len() as i64;
                    buffer.write(&bytes)?;
                    n
                }
                None => {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
        })
    }

    pub(super) fn serve_write(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Write { fd, bytes } = call else {
            unreachable!("dispatched serve_write")
        };
        Ok({
            match self.virtual_write(fd, &bytes) {
                Some(count) => count as i64,
                None => {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
        })
    }

    pub(super) fn serve_read_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::ReadAt {
            fd,
            buffer,
            count,
            offset,
        } = call
        else {
            unreachable!("dispatched serve_read_at")
        };
        Ok({
            // `pread(fd, buf, count, offset)`: read at an absolute offset
            // WITHOUT moving the cursor (Rust `FileExt::read_at`).
            match self.virtual_read_at(fd, offset, count.host) {
                Some(bytes) => {
                    let n = bytes.len() as i64;
                    buffer.write(&bytes)?;
                    n
                }
                None => {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
        })
    }

    pub(super) fn serve_write_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::WriteAt { fd, bytes, offset } = call else {
            unreachable!("dispatched serve_write_at")
        };
        Ok({
            // `pwrite(fd, buf, count, offset)`: write at an absolute offset
            // WITHOUT moving the cursor (Rust `FileExt::write_at`).
            match self.virtual_write_at(fd, offset, &bytes) {
                Some(count) => count as i64,
                None => {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
        })
    }

    pub(super) fn serve_close(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Close { fd } = call else {
            unreachable!("dispatched serve_close")
        };
        Ok({
            if self.virtual_fds.remove(&fd).is_some() {
                // Closing the owning fd releases any advisory lock it held.
                self.virtual_flocks.retain(|_, owner| *owner != fd);
                0
            } else {
                self.virtual_errno = 9; // EBADF
                -1
            }
        })
    }

    pub(super) fn serve_close_handle(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::CloseHandle { handle } = call else {
            unreachable!("dispatched serve_close_handle")
        };
        Ok({
            match synthetic_handle_fd(handle) {
                Some(handle) if self.virtual_fds.remove(&handle).is_some() => {
                    self.virtual_flocks.retain(|_, owner| *owner != handle);
                    1 // Win32 BOOL success
                }
                _ => {
                    self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                    0
                }
            }
        })
    }

    pub(super) fn serve_duplicate(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Duplicate { fd } = call else {
            unreachable!("dispatched serve_duplicate")
        };
        Ok({
            // `dup(fd)`: mint a fresh descriptor over the same open file (Rust
            // `File::try_clone`). Both descriptor lifetimes share the same
            // open-file-description cursor. EBADF for an unknown fd.
            let clone = self.virtual_fds.get(&fd).map(|descriptor| VirtualFd {
                path: descriptor.path.clone(),
                cursor: std::rc::Rc::clone(&descriptor.cursor),
                writable: descriptor.writable,
                is_dir: descriptor.is_dir,
            });
            match clone {
                Some(clone) => {
                    let new_fd = self.virtual_next_fd;
                    self.virtual_next_fd += 1;
                    self.virtual_fds.insert(new_fd, clone);
                    new_fd as i64
                }
                None => {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
        })
    }

    pub(super) fn serve_lock_file(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::LockFile { fd, operation } = call else {
            unreachable!("dispatched serve_lock_file")
        };
        Ok({
            // `flock(fd, operation)`: advisory whole-file lock (Rust
            // `File::lock`/`lock_shared`/`try_lock`/`unlock`). operation
            // bitmask: LOCK_SH=1, LOCK_EX=2, LOCK_NB=4, LOCK_UN=8. The
            // hermetic model tracks EXCLUSIVE ownership per path; a
            // non-blocking acquire on a path another fd holds is EWOULDBLOCK.
            let path = self
                .virtual_fds
                .get(&fd)
                .map(|descriptor| descriptor.path.clone());
            match path {
                None => {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
                Some(path) if operation & 8 != 0 => {
                    // LOCK_UN: release this fd's lock (a no-op if it held none).
                    if self.virtual_flocks.get(&path) == Some(&fd) {
                        self.virtual_flocks.remove(&path);
                    }
                    0
                }
                Some(path) => {
                    let held_by_other = matches!(
                        self.virtual_flocks.get(&path),
                        Some(owner) if *owner != fd
                    );
                    if held_by_other && operation & 4 != 0 {
                        self.virtual_errno = 35; // EWOULDBLOCK (== EAGAIN)
                        -1
                    } else {
                        self.virtual_flocks.insert(path, fd);
                        0
                    }
                }
            }
        })
    }

    pub(super) fn serve_lock_file_ex(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::LockFileEx {
            handle,
            flags,
            reserved: _,
            length_low: _,
            length_high: _,
            overlapped: _,
        } = call
        else {
            unreachable!("dispatched serve_lock_file_ex")
        };
        Ok({
            // Win32 LockFileEx over the synthetic fd/HANDLE. flags:
            // EXCLUSIVE=2, FAIL_IMMEDIATELY=1. The range/OVERLAPPED
            // arguments are ABI-shape inputs; the std wrapper always asks
            // for offset zero and the whole file.
            let Some(fd) = synthetic_handle_fd(handle) else {
                self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                return Ok(0);
            };
            let flags = flags as i32;
            let path = self
                .virtual_fds
                .get(&fd)
                .map(|descriptor| descriptor.path.clone());
            match path {
                None => {
                    self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                    0
                }
                Some(path) => {
                    let held_by_other = matches!(
                        self.virtual_flocks.get(&path),
                        Some(owner) if *owner != fd
                    );
                    if held_by_other && flags & 1 != 0 {
                        self.virtual_errno = 33; // ERROR_LOCK_VIOLATION
                        0
                    } else {
                        self.virtual_flocks.insert(path, fd);
                        1
                    }
                }
            }
        })
    }

    pub(super) fn serve_unlock_file(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::UnlockFile {
            handle,
            offset_low: _,
            offset_high: _,
            length_low: _,
            length_high: _,
        } = call
        else {
            unreachable!("dispatched serve_unlock_file")
        };
        Ok({
            let Some(fd) = synthetic_handle_fd(handle) else {
                self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                return Ok(0);
            };
            let path = self
                .virtual_fds
                .get(&fd)
                .map(|descriptor| descriptor.path.clone());
            match path {
                None => {
                    self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                    0
                }
                Some(path) if self.virtual_flocks.get(&path) == Some(&fd) => {
                    self.virtual_flocks.remove(&path);
                    1
                }
                Some(_) => {
                    self.virtual_errno = 158; // ERROR_NOT_LOCKED
                    0
                }
            }
        })
    }

    pub(super) fn serve_seek(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Seek { fd, offset, whence } = call else {
            unreachable!("dispatched serve_seek")
        };
        Ok({
            match self.virtual_seek(fd, offset, whence) {
                Some(position) => position,
                None => {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
        })
    }

    pub(super) fn serve_set_len(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::SetLen { fd, length } = call else {
            unreachable!("dispatched serve_set_len")
        };
        Ok({
            let rc = self.virtual_set_len(fd, length);
            if rc < 0 {
                self.virtual_errno = 9; // EBADF
            }
            rc
        })
    }

    pub(super) fn serve_set_file_permissions(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::SetFilePermissions { fd, mode } = call else {
            unreachable!("dispatched serve_set_file_permissions")
        };
        Ok({
            // `fchmod(fd, mode)`: record the mode against the fd's path so a
            // subsequent write-open sees it (mirrors path-based chmod). EBADF
            // if the descriptor is unknown.
            match self.virtual_fds.get(&fd) {
                Some(descriptor) => {
                    let path = descriptor.path.clone();
                    self.virtual_perms.insert(path, mode);
                    0
                }
                None => {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
        })
    }

    pub(super) fn serve_set_file_times(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::SetFileTimes { fd, times } = call else {
            unreachable!("dispatched serve_set_file_times")
        };
        Ok({
            // `futimens(fd, times)`: `times` is two packed `struct timespec`
            // (atime then mtime, {tv_sec i64, tv_nsec i64} each). Read the
            // modification seconds -- times[1].tv_sec at byte offset 16 -- and
            // record it against the fd's path so stat/fstat report it. EBADF if
            // the descriptor is unknown.
            match self.virtual_fds.get(&fd) {
                Some(descriptor) => {
                    let path = descriptor.path.clone();
                    let mtime = times
                        .bytes
                        .get(16..24)
                        .and_then(|s| <[u8; 8]>::try_from(s).ok())
                        .map(i64::from_le_bytes)
                        .unwrap_or(0);
                    self.virtual_times.insert(path, mtime);
                    0
                }
                None => {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
        })
    }

    pub(super) fn serve_sync(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let (PreparedFilesystemCall::Sync { fd } | PreparedFilesystemCall::SyncData { fd }) = call
        else {
            unreachable!("dispatched serve_sync")
        };
        Ok({
            // `fsync(fd)`: flush to durable storage (`sync_data` aliases it --
            // macOS has no `fdatasync`). In the hermetic in-memory FS the bytes
            // are already "durable", so this is a no-op that only validates the
            // descriptor: 0 for a live fd, -1 (EBADF) otherwise -- matching the
            // native seam's contract.
            if self.virtual_fds.contains_key(&fd) {
                0
            } else {
                self.virtual_errno = 9; // EBADF
                -1
            }
        })
    }

    pub(super) fn serve_change_file_owner(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::ChangeFileOwner { fd, uid, gid } = call else {
            unreachable!("dispatched serve_change_file_owner")
        };
        Ok({
            // `fchown(fd, uid, gid)`: like `chown` by descriptor. EBADF for an
            // unknown fd; otherwise the same non-root ownership rule.
            if self.virtual_fds.contains_key(&fd) {
                self.virtual_chown_result(uid, gid)
            } else {
                self.virtual_errno = 9; // EBADF
                -1
            }
        })
    }

    pub(super) fn serve_get_osf_handle(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::GetOsfHandle { fd } = call else {
            unreachable!("dispatched serve_get_osf_handle")
        };
        Ok({
            // `_get_osfhandle(fd)` -- the fd -> HANDLE bridge (session
            // slice 4a). The hermetic model's handles ARE its fds
            // (identity), so consumers key the same descriptor table;
            // -2 (msvcrt's bad-fd spelling) for an unknown fd.
            if self.virtual_fds.contains_key(&fd) {
                i64::from(fd)
            } else {
                -2
            }
        })
    }

    pub(super) fn serve_final_path_name_by_handle(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::FinalPathNameByHandle {
            handle,
            buffer,
            capacity,
            flags: _,
        } = call
        else {
            unreachable!("dispatched serve_final_path_name_by_handle")
        };
        Ok({
            // `GetFinalPathNameByHandleA(handle, buffer, capacity, flags)`:
            // resolve an OPEN handle to its final path. The hermetic
            // model's canonical path IS the descriptor's stored key
            // (already absolute for its namespace; no drive letters or
            // \\?\ prefixes to synthesize), NUL-terminated into the
            // buffer. Win32 return contract: the length WITHOUT the NUL
            // when it fits, the REQUIRED size INCLUDING the NUL when the
            // capacity is too small, 0 for a bad handle (GetLastError
            // semantics -- no errno touched).
            let path = synthetic_handle_fd(handle)
                .and_then(|handle| self.virtual_fds.get(&handle))
                .map(|descriptor| descriptor.path.clone());
            match path {
                Some(path) => {
                    if path.len() < capacity.host {
                        let mut bytes = path.clone();
                        bytes.push(0);
                        buffer.write(&bytes)?;
                        path.len() as i64
                    } else {
                        (path.len() + 1) as i64
                    }
                }
                None => {
                    self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                    0
                }
            }
        })
    }

    pub(super) fn serve_set_file_time(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::SetFileTime {
            handle,
            creation: _,
            last_access: _,
            last_write: write_ft,
        } = call
        else {
            unreachable!("dispatched serve_set_file_time")
        };
        Ok({
            // `SetFileTime(handle, creation, access_ft, write_ft)` (session
            // slice 4b): stamp the handle's path with the WRITE time from
            // its 8-byte FILETIME buffer (100ns units since 1601 -> unix
            // seconds via the calibration constants), the same
            // virtual_times store `set_file_times` uses. BOOL result;
            // 0 for a bad handle (GetLastError semantics -- no errno).
            match synthetic_handle_fd(handle).and_then(|handle| self.virtual_fds.get(&handle)) {
                Some(descriptor) => {
                    let path = descriptor.path.clone();
                    let filetime = write_ft
                        .get(0..8)
                        .and_then(|s| <[u8; 8]>::try_from(s).ok())
                        .map(i64::from_le_bytes)
                        .unwrap_or(0);
                    let secs = filetime / 10_000_000 - 11_644_473_600;
                    self.virtual_times.insert(path, secs);
                    1
                }
                None => {
                    self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                    0
                }
            }
        })
    }

    pub(super) fn serve_read_file_metadata(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::ReadFileMetadata { fd, buffer } = call else {
            unreachable!("dispatched serve_read_file_metadata")
        };
        Ok({
            // `fstat(fd, buf)`: like `stat` but keyed by an OPEN descriptor. Map
            // the fd to its path, then fill the same stat record (a held `File`
            // is always a regular file here). EBADF for an unknown fd. Never
            // touches the cursor.
            let path = self
                .virtual_fds
                .get(&fd)
                .map(|descriptor| descriptor.path.clone());
            let meta = path.and_then(|path| {
                // A `set_file_times` mtime shows through; else the modeled epoch.
                let mtime = self
                    .virtual_times
                    .get(&path)
                    .copied()
                    .unwrap_or(VIRTUAL_MTIME_SECS);
                let chmod_perm = self
                    .virtual_perms
                    .get(&path)
                    .map(|mode| (*mode as u16) & 0o7777);
                if let Some(content) = self.virtual_files.get(&path) {
                    Some((
                        0o100_000u16 | chmod_perm.unwrap_or(0o644),
                        content.len() as i64,
                        mtime,
                    ))
                } else if self.virtual_dirs.contains(&path) {
                    Some((0o040_000u16 | chmod_perm.unwrap_or(0o755), 0i64, mtime))
                } else {
                    None
                }
            });
            match meta {
                Some((mode, size, mtime)) => {
                    self.write_fs_stat(
                        &buffer,
                        FilesystemMetadataObservationKind::OpenDescriptor,
                        u32::from(mode),
                        size,
                        mtime,
                    )?;
                    0
                }
                None => {
                    self.virtual_errno = 9; // EBADF (unknown descriptor)
                    -1
                }
            }
        })
    }
}
