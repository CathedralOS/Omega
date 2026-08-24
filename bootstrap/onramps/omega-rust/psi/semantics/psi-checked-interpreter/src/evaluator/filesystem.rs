use super::*;

/// Deterministic per-operation allocation ceiling for raw filesystem reads.
/// This is an evaluator sponsor limit, not a language or OS API limit. A future
/// build policy may supply a stricter budget, but package code cannot raise it.
const MAX_FILESYSTEM_TRANSFER_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilesystemTransferCountError {
    NegativeOrUnrepresentable,
    ExceedsEvaluatorLimit,
}

fn checked_filesystem_transfer_count(raw: i64) -> Result<usize, FilesystemTransferCountError> {
    let count = usize::try_from(raw)
        .map_err(|_| FilesystemTransferCountError::NegativeOrUnrepresentable)?;
    if count > MAX_FILESYSTEM_TRANSFER_BYTES {
        return Err(FilesystemTransferCountError::ExceedsEvaluatorLimit);
    }
    Ok(count)
}

impl<'program> Evaluator<'program> {
    /// Record one exact canonical operation in call-start order around the
    /// selected provider. The placeholder preserves nesting order if argument
    /// evaluation itself invokes another filesystem operation. A provider
    /// evaluation halt aborts the entire build, so incomplete placeholders can
    /// never enter a successful measured result.
    pub(super) fn try_filesystem_call(
        &mut self,
        operation: FilesystemHostOperation,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<Value> {
        let provider = match self.real_fs.as_ref() {
            None => FilesystemObservationProvider::Virtual,
            Some(filesystem) if filesystem.is_scoped() => FilesystemObservationProvider::RealScoped,
            Some(_) => FilesystemObservationProvider::RealUnscoped,
        };
        let attempt_index = self.filesystem_operation_attempts.len();
        self.filesystem_operation_attempts
            .push(FilesystemOperationAttempt::pending(
                operation.operation_tag(),
                provider,
            ));
        let outcome = self.serve_filesystem_call(operation, arguments, frame);
        if let Ok(value) = &outcome {
            let result = value.as_int().ok_or_else(|| {
                Halt::Trap(format!(
                    "canonical filesystem operation `{operation}` returned a non-integer value"
                ))
            })?;
            let post_error = self
                .real_fs
                .as_ref()
                .map_or(self.virtual_errno, |filesystem| filesystem.errno);
            self.filesystem_operation_attempts[attempt_index] = FilesystemOperationAttempt {
                operation_tag: operation.operation_tag(),
                provider,
                result,
                post_error,
            };
        }
        outcome
    }

    /// Drive a value-returning `FilesystemHost` operation against the selected
    /// filesystem provider. Argument expressions remain table handles and are
    /// evaluated inside the selected provider arm; evidence capture must not
    /// evaluate them a second time. The closed operation type makes dispatch
    /// exhaustive.
    fn serve_filesystem_call(
        &mut self,
        operation: FilesystemHostOperation,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<Value> {
        // REAL-filesystem mode (build.omg rung; opt-in via
        // `FilesystemAccess::RealUnscoped`): the whole op family routes to the
        // real provider with the same exhaustive operation set.
        if self.real_fs.is_some() {
            return self.try_real_filesystem_call(operation, arguments, frame);
        }
        // Value-returning raw `FilesystemHost` ops, matching the native seam:
        // each returns its "syscall" result (fd / byte count / rc; negative on
        // error) against the deterministic in-memory filesystem.
        let result: i64 = match operation {
            FilesystemHostOperation::Create => {
                // O_WRONLY|O_CREAT|O_TRUNC: create/truncate, writable.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                self.virtual_open(path, true, true) as i64
            }
            FilesystemHostOperation::Open => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let flags = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                self.virtual_open_flags(path, flags) as i64
            }
            FilesystemHostOperation::OpenPathHandle => {
                // Hermetic CreateFileA model for metadata/query handles. The
                // wrapper supplies access=0 + OPEN_EXISTING; the virtual fd
                // table already models both files and read-only directories.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let fd = self.virtual_open_flags(path, 0);
                if fd < 0 {
                    // `GetLastError`, not CRT errno, is the native error source.
                    self.virtual_errno = match self.virtual_errno {
                        13 => 5,   // EACCES -> ERROR_ACCESS_DENIED
                        9 => 6,    // EBADF -> ERROR_INVALID_HANDLE
                        17 => 183, // EEXIST -> ERROR_ALREADY_EXISTS
                        _ => 2,    // ERROR_FILE_NOT_FOUND
                    };
                }
                fd as i64
            }
            FilesystemHostOperation::OpenCreate => {
                // `open(path, flags, mode)` with O_CREAT (Rust `File::create_new`,
                // `OpenOptions.create`/`.create_new`). Flag bits are the HOST's
                // (host_open_flags, mirroring the checked target encoder). This
                // adds the O_EXCL/EEXIST atomic
                // create-new guard + create-mode recording; every other flag bit
                // (O_TRUNC/O_APPEND/access/EACCES/ENOENT) is handled by the shared
                // `virtual_open_flags`, so `open_create` cleanly SUBSUMES `open`.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let flags = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let mode = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as u32;
                let exists = self.virtual_files.contains_key(&path)
                    || self.virtual_dirs.contains(&path)
                    || self.virtual_char_devices.contains(&path);
                if host_open_flags::o_creat(flags) && host_open_flags::o_excl(flags) && exists {
                    self.virtual_errno = 17; // EEXIST (O_CREAT|O_EXCL, path present)
                    -1
                } else {
                    // Whether this call actually creates the file (records the mode
                    // AFTER the open so the create's own access is not gated by it).
                    let created = host_open_flags::o_creat(flags) && !exists;
                    let fd = self.virtual_open_flags(path.clone(), flags);
                    if fd >= 0 && created {
                        self.virtual_perms.insert(path, mode & 0o777);
                    }
                    fd as i64
                }
            }
            FilesystemHostOperation::Read => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let count = self.eval_fs_transfer_count(arguments.get(2).copied(), frame)?;
                match self.virtual_read_n(fd, count) {
                    Some(bytes) => {
                        let n = bytes.len() as i64;
                        self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
                        n
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            FilesystemHostOperation::Write => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let bytes = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                match self.virtual_write(fd, &bytes) {
                    Some(count) => count as i64,
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            FilesystemHostOperation::ReadAt => {
                // `pread(fd, buf, count, offset)`: read at an absolute offset
                // WITHOUT moving the cursor (Rust `FileExt::read_at`).
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let count = self.eval_fs_transfer_count(arguments.get(2).copied(), frame)?;
                let offset = self.eval_fs_scalar(arguments.get(3).copied(), frame)?;
                match self.virtual_read_at(fd, offset, count) {
                    Some(bytes) => {
                        let n = bytes.len() as i64;
                        self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
                        n
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            FilesystemHostOperation::WriteAt => {
                // `pwrite(fd, buf, count, offset)`: write at an absolute offset
                // WITHOUT moving the cursor (Rust `FileExt::write_at`).
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let bytes = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                let offset = self.eval_fs_scalar(arguments.get(2).copied(), frame)?;
                match self.virtual_write_at(fd, offset, &bytes) {
                    Some(count) => count as i64,
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            FilesystemHostOperation::Close => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                if self.virtual_fds.remove(&fd).is_some() {
                    // Closing the owning fd releases any advisory lock it held.
                    self.virtual_flocks.retain(|_, owner| *owner != fd);
                    0
                } else {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
            FilesystemHostOperation::CloseHandle => {
                let handle = self.eval_fs_fd(arguments.first().copied(), frame)?;
                if self.virtual_fds.remove(&handle).is_some() {
                    self.virtual_flocks.retain(|_, owner| *owner != handle);
                    1 // Win32 BOOL success
                } else {
                    self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                    0
                }
            }
            FilesystemHostOperation::Duplicate => {
                // `dup(fd)`: mint a fresh descriptor over the same open file (Rust
                // `File::try_clone`). Native dup SHARES the underlying file offset;
                // the hermetic model gives the clone its OWN cursor snapshotted from
                // the source (independent thereafter) -- faithful for the common
                // clone-then-use pattern, where the clone's offset starts where the
                // source's was. EBADF for an unknown fd.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let clone = self.virtual_fds.get(&fd).map(|descriptor| VirtualFd {
                    path: descriptor.path.clone(),
                    cursor: descriptor.cursor,
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
            }
            FilesystemHostOperation::LockFile => {
                // `flock(fd, operation)`: advisory whole-file lock (Rust
                // `File::lock`/`lock_shared`/`try_lock`/`unlock`). operation
                // bitmask: LOCK_SH=1, LOCK_EX=2, LOCK_NB=4, LOCK_UN=8. The
                // hermetic model tracks EXCLUSIVE ownership per path; a
                // non-blocking acquire on a path another fd holds is EWOULDBLOCK.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let operation = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
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
            }
            FilesystemHostOperation::LockFileEx => {
                // Win32 LockFileEx over the synthetic fd/HANDLE. flags:
                // EXCLUSIVE=2, FAIL_IMMEDIATELY=1. The range/OVERLAPPED
                // arguments are ABI-shape inputs; the std wrapper always asks
                // for offset zero and the whole file.
                let fd = self.eval_fs_scalar(arguments.first().copied(), frame)? as i32;
                let flags = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
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
            }
            FilesystemHostOperation::UnlockFile => {
                let fd = self.eval_fs_scalar(arguments.first().copied(), frame)? as i32;
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
            }
            FilesystemHostOperation::GetLastError => i64::from(self.virtual_errno),
            // `remove_name` is the TRUSTED plain-path twin (D-at trust class,
            // the create_dir_name precedent): the arg bytes ARE the path, so
            // both spellings share one model.
            FilesystemHostOperation::Remove | FilesystemHostOperation::RemoveName => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                if self.virtual_files.remove(&path).is_some() {
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            FilesystemHostOperation::Seek => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let offset = self.eval_fs_scalar(arguments.get(1).copied(), frame)?;
                let whence = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                match self.virtual_seek(fd, offset, whence) {
                    Some(position) => position,
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            FilesystemHostOperation::SetLen => {
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let length = self.eval_fs_scalar(arguments.get(1).copied(), frame)?;
                let rc = self.virtual_set_len(fd, length);
                if rc < 0 {
                    self.virtual_errno = 9; // EBADF
                }
                rc
            }
            FilesystemHostOperation::SetFilePermissions => {
                // `fchmod(fd, mode)`: record the mode against the fd's path so a
                // subsequent write-open sees it (mirrors path-based chmod). EBADF
                // if the descriptor is unknown.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let mode = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as u32;
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
            }
            FilesystemHostOperation::SetFileTimes => {
                // `futimens(fd, times)`: `times` is two packed `struct timespec`
                // (atime then mtime, {tv_sec i64, tv_nsec i64} each). Read the
                // modification seconds -- times[1].tv_sec at byte offset 16 -- and
                // record it against the fd's path so stat/fstat report it. EBADF if
                // the descriptor is unknown.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let times = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                match self.virtual_fds.get(&fd) {
                    Some(descriptor) => {
                        let path = descriptor.path.clone();
                        let mtime = times
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
            }
            FilesystemHostOperation::Sync | FilesystemHostOperation::SyncData => {
                // `fsync(fd)`: flush to durable storage (`sync_data` aliases it --
                // macOS has no `fdatasync`). In the hermetic in-memory FS the bytes
                // are already "durable", so this is a no-op that only validates the
                // descriptor: 0 for a live fd, -1 (EBADF) otherwise -- matching the
                // native seam's contract.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                i64::from(self.virtual_fds.contains_key(&fd)) - 1
            }
            FilesystemHostOperation::Errno => {
                // `read_errno()` (darwin `___error()` deref): the thread-local
                // errno set by the most recent failing op. Not cleared on
                // success (POSIX), so it is only meaningful right after a -1.
                i64::from(self.virtual_errno)
            }
            // The trusted plain-name variant shares create_dir's semantics
            // (the arg bytes ARE the path -- the scratch subslice excludes
            // the native NUL, so both engines see identical bytes).
            FilesystemHostOperation::CreateDir | FilesystemHostOperation::CreateDirName => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                // -1 (EEXIST) if the dir already exists.
                if self.virtual_dirs.insert(path) {
                    0
                } else {
                    self.virtual_errno = 17; // EEXIST
                    -1
                }
            }
            FilesystemHostOperation::RemoveDir | FilesystemHostOperation::RemoveDirName => {
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                if self.virtual_dirs.remove(&path) {
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            FilesystemHostOperation::OpenAt => {
                // `openat(dirfd, name, flags)`: open `name` relative to the open
                // directory `dirfd`. The full path (dirfd's path + "/" + name) is
                // joined HERE (the OS does it natively), so no Omega path build.
                let dirfd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let name = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                let flags = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                match self.virtual_at_path(dirfd, &name) {
                    Some(full) => self.virtual_open_flags(full, flags) as i64,
                    None => {
                        self.virtual_errno = 9; // EBADF (dirfd not an open directory)
                        -1
                    }
                }
            }
            FilesystemHostOperation::UnlinkAt => {
                // `unlinkat(dirfd, name, flags)`: remove `name` relative to `dirfd`.
                // flags & AT_REMOVEDIR(0x80) removes an empty directory, else a file.
                let dirfd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let name = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                let flags = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                match self.virtual_at_path(dirfd, &name) {
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                    Some(full) => {
                        let removed = if (flags & 128) != 0 {
                            self.virtual_dirs.remove(&full)
                        } else {
                            self.virtual_files.remove(&full).is_some()
                        };
                        if removed {
                            0
                        } else {
                            self.virtual_errno = 2; // ENOENT
                            -1
                        }
                    }
                }
            }
            FilesystemHostOperation::SetPermissions => {
                // `chmod(path, mode)`: record the mode. ENOENT if the path names
                // neither a file nor a directory. `mode` is the second arg.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let mode = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as u32;
                if self.virtual_files.contains_key(&path) || self.virtual_dirs.contains(&path) {
                    self.virtual_perms.insert(path, mode);
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            FilesystemHostOperation::ChangeOwner | FilesystemHostOperation::ChangeOwnerNoFollow => {
                // `chown`/`lchown(path, uid, gid)`: change owner/group. ENOENT if
                // the path is absent. The hermetic model's process identity is
                // VIRTUAL_UID/GID (a normal, non-root user), so only a NO-OP change
                // is permitted: a uid/gid of -1 leaves that component alone, and
                // setting the CURRENT owner succeeds; any OTHER owner is EPERM --
                // exactly what native `chown` does when run as a normal user.
                // (`lchown` differs from `chown` only on symlinks, which the
                // hermetic FS never follows on ownership ops, so they behave
                // identically here.)
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let uid = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let gid = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                let exists = self.virtual_files.contains_key(&path)
                    || self.virtual_dirs.contains(&path)
                    || self.virtual_symlinks.contains_key(&path);
                if !exists {
                    self.virtual_errno = 2; // ENOENT
                    -1
                } else {
                    self.virtual_chown_result(uid, gid)
                }
            }
            FilesystemHostOperation::ChangeFileOwner => {
                // `fchown(fd, uid, gid)`: like `chown` by descriptor. EBADF for an
                // unknown fd; otherwise the same non-root ownership rule.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let uid = self.eval_fs_scalar(arguments.get(1).copied(), frame)? as i32;
                let gid = self.eval_fs_scalar(arguments.get(2).copied(), frame)? as i32;
                if self.virtual_fds.contains_key(&fd) {
                    self.virtual_chown_result(uid, gid)
                } else {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
            FilesystemHostOperation::Rename => {
                let from = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let to = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                match self.virtual_files.remove(&from) {
                    Some(content) => {
                        self.virtual_files.insert(to, content);
                        0
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            FilesystemHostOperation::HardLink => {
                // `link(original, link)`: a second name for the same inode.
                // ENOENT if the original is absent; EEXIST if the link name is
                // taken. The hermetic FS has no inodes, so this COPIES the bytes
                // (approximate: a later write to one name won't show in the
                // other — see TASKS_FS.md). Enough to model create+readback.
                let original = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let link = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                if self.virtual_files.contains_key(&link) || self.virtual_dirs.contains(&link) {
                    self.virtual_errno = 17; // EEXIST
                    -1
                } else if let Some(content) = self.virtual_files.get(&original).cloned() {
                    self.virtual_files.insert(link, content);
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            FilesystemHostOperation::CreateHardLink => {
                // `CreateHardLinkA(link, existing, security)` -- the WINDOWS
                // hard-link primitive (session slice 3): the ARG ORDER is
                // (new link, existing), REVERSED from `hard_link`, and the
                // result is BOOL (1 success / 0 failure). Same hermetic
                // copy-the-bytes model as `hard_link` above. virtual_errno is
                // also the provider's Win32 last-error slot for GetLastError.
                let link = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let existing = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                if self.virtual_files.contains_key(&link) || self.virtual_dirs.contains(&link) {
                    self.virtual_errno = 183; // ERROR_ALREADY_EXISTS
                    0
                } else if let Some(content) = self.virtual_files.get(&existing).cloned() {
                    self.virtual_files.insert(link, content);
                    1
                } else {
                    self.virtual_errno = 2; // ERROR_FILE_NOT_FOUND
                    0
                }
            }
            FilesystemHostOperation::GetOsfHandle => {
                // `_get_osfhandle(fd)` -- the fd -> HANDLE bridge (session
                // slice 4a). The hermetic model's handles ARE its fds
                // (identity), so consumers key the same descriptor table;
                // -2 (msvcrt's bad-fd spelling) for an unknown fd.
                let fd = self.eval_fs_scalar(arguments.first().copied(), frame)? as i32;
                if self.virtual_fds.contains_key(&fd) {
                    i64::from(fd)
                } else {
                    -2
                }
            }
            FilesystemHostOperation::FinalPathNameByHandle => {
                // `GetFinalPathNameByHandleA(handle, buffer, capacity, flags)`:
                // resolve an OPEN handle to its final path. The hermetic
                // model's canonical path IS the descriptor's stored key
                // (already absolute for its namespace; no drive letters or
                // \\?\ prefixes to synthesize), NUL-terminated into the
                // buffer. Win32 return contract: the length WITHOUT the NUL
                // when it fits, the REQUIRED size INCLUDING the NUL when the
                // capacity is too small, 0 for a bad handle (GetLastError
                // semantics -- no errno touched).
                let handle = self.eval_fs_scalar(arguments.first().copied(), frame)?;
                let capacity = self.eval_fs_transfer_count(arguments.get(2).copied(), frame)?;
                let path = self
                    .virtual_fds
                    .get(&(handle as i32))
                    .map(|descriptor| descriptor.path.clone());
                match path {
                    Some(path) => {
                        if path.len() + 1 <= capacity {
                            let mut bytes = path.clone();
                            bytes.push(0);
                            self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
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
            }
            FilesystemHostOperation::SetFileTime => {
                // `SetFileTime(handle, creation, access_ft, write_ft)` (session
                // slice 4b): stamp the handle's path with the WRITE time from
                // its 8-byte FILETIME buffer (100ns units since 1601 -> unix
                // seconds via the calibration constants), the same
                // virtual_times store `set_file_times` uses. BOOL result;
                // 0 for a bad handle (GetLastError semantics -- no errno).
                let handle = self.eval_fs_scalar(arguments.first().copied(), frame)?;
                let write_ft = self.eval_fs_bytes(arguments.get(3).copied(), frame)?;
                match self.virtual_fds.get(&(handle as i32)) {
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
            }
            FilesystemHostOperation::Symlink => {
                // `symlink(target, linkpath)`: record the link -> target mapping.
                // EEXIST if the link name already names a file/dir/symlink.
                let target = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let link = self.eval_fs_bytes(arguments.get(1).copied(), frame)?;
                if self.virtual_files.contains_key(&link)
                    || self.virtual_dirs.contains(&link)
                    || self.virtual_symlinks.contains_key(&link)
                {
                    self.virtual_errno = 17; // EEXIST
                    -1
                } else {
                    self.virtual_symlinks.insert(link, target);
                    0
                }
            }
            FilesystemHostOperation::ReadLink => {
                // `readlink(path, buf, count)`: write the target bytes into the
                // buffer (up to `count`), returning the number written. ENOENT if
                // `path` is not a symlink in the hermetic model.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let count = self.eval_fs_transfer_count(arguments.get(2).copied(), frame)?;
                match self.virtual_symlinks.get(&path).cloned() {
                    Some(target) => {
                        let n = target.len().min(count);
                        self.write_fs_buffer(arguments.get(1).copied(), frame, &target[..n]);
                        n as i64
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            FilesystemHostOperation::Canonicalize => {
                // `realpath(path, buf)`: resolve `path` to its canonical absolute
                // form and write it NUL-terminated into the buffer. The hermetic FS
                // is already absolute and does not resolve `.`/`..`; it follows one
                // symlink level (matching `read_link`). Returns a non-zero success
                // flag (native returns the resolved-buffer pointer) or 0 (NULL) +
                // ENOENT when the target does not exist.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let resolved = self.virtual_symlinks.get(&path).cloned().unwrap_or(path);
                let exists = self.virtual_files.contains_key(&resolved)
                    || self.virtual_dirs.contains(&resolved);
                if exists {
                    let mut bytes = resolved;
                    bytes.push(0); // NUL-terminate like realpath's C string
                    self.write_fs_buffer(arguments.get(1).copied(), frame, &bytes);
                    1
                } else {
                    self.virtual_errno = 2; // ENOENT
                    0
                }
            }
            FilesystemHostOperation::ReadDir => {
                // `read_dir(fd, buf, count, &position)`: pack the directory's
                // entries as Darwin `dirent` records and return the next window
                // of complete records. `position` is a synthetic byte cursor, so
                // repeated calls drain directories larger than one buffer just
                // like native `___getdirentries64`.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
                let dir_path = self
                    .virtual_fds
                    .get(&fd)
                    .filter(|descriptor| descriptor.is_dir)
                    .map(|descriptor| descriptor.path.clone());
                match dir_path {
                    None => {
                        // Unknown fd -> EBADF; a live non-dir fd -> ENOTDIR.
                        self.virtual_errno = if self.virtual_fds.contains_key(&fd) {
                            20 // ENOTDIR
                        } else {
                            9 // EBADF
                        };
                        -1
                    }
                    Some(path) => {
                        let count =
                            self.eval_fs_transfer_count(arguments.get(2).copied(), frame)?;
                        let position = self.read_fs_position(arguments.get(3).copied(), frame);
                        let records = self.build_dirent_records(&path);
                        let start = position.max(0) as usize;
                        let (chunk, next_position) = dirent_record_chunk(&records, start, count);
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
                }
            }
            FilesystemHostOperation::FindFirst => {
                // `find_first(pattern, &data)` -- the windows dir-walk seam (fs
                // rung 3a). `pattern` is `dir/*`: the impl joins with `/`, which
                // Win32 accepts natively and which matches the hermetic FS keys
                // byte-exactly. Snapshot the directory's entries (".", "..",
                // then the immediate children -- the same set read_dir packs)
                // into a cursor keyed by a fresh handle, fill the FIRST entry's
                // find-data record, and return the handle; -1
                // (INVALID_HANDLE_VALUE, ENOENT) when the directory does not
                // exist. A real directory always yields "." first, so an open
                // enumeration always has a first entry -- exactly Win32.
                let pattern = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let entries = pattern
                    .strip_suffix(b"/*")
                    .filter(|dir_path| self.virtual_dirs.contains(*dir_path))
                    .map(|dir_path| self.build_find_entries(dir_path));
                match entries {
                    Some(mut entries) => {
                        let (name, is_dir) =
                            entries.pop_front().expect("dot entries are always present");
                        self.write_find_data(arguments.get(1).copied(), frame, &name, is_dir);
                        let handle = self.virtual_next_find;
                        self.virtual_next_find += 1;
                        self.virtual_finds.insert(handle, entries);
                        handle
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            FilesystemHostOperation::FindNext => {
                // `find_next(handle, &data)`: fill the next snapshotted entry
                // (1 = filled, 0 = end-of-enumeration or unknown handle).
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
                // `find_close(handle)`: release the cursor (BOOL, like Win32).
                let handle = self.eval_fs_scalar(arguments.first().copied(), frame)?;
                if self.virtual_finds.remove(&handle).is_some() {
                    1
                } else {
                    0
                }
            }
            FilesystemHostOperation::ReadMetadata => {
                // `stat(path, buf)`: fill the buffer's st_mode (off 4, u16) and
                // st_size (off 96, i64) as the darwin kernel would. A regular
                // file is S_IFREG(0o100000)|0o644 with size = content length; a
                // directory is S_IFDIR(0o040000)|0o755 size 0. ENOENT otherwise.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                // st_mode = format bits (S_IFREG/S_IFDIR) | permission bits, so
                // a prior `set_permissions` (chmod) shows through `readonly()`.
                let chmod_perm = self
                    .virtual_perms
                    .get(&path)
                    .map(|mode| (*mode as u16) & 0o7777);
                let meta = if self.virtual_char_devices.contains(&path) {
                    // A character-special device (`/dev/null`): S_IFCHR|0o666, size 0.
                    Some((0o020_000u16 | chmod_perm.unwrap_or(0o666), 0i64))
                } else if let Some(content) = self.virtual_files.get(&path) {
                    let size = content.len() as i64;
                    Some((0o100_000u16 | chmod_perm.unwrap_or(0o644), size))
                } else if self.virtual_dirs.contains(&path) {
                    Some((0o040_000u16 | chmod_perm.unwrap_or(0o755), 0i64))
                } else {
                    None
                };
                match meta {
                    Some((mode, size)) => {
                        // A `set_file_times` mtime shows through; otherwise the
                        // hermetic FS has no clock, so it reports a fixed modeled
                        // mtime (native `stat` returns the real time -- tests assert
                        // exact == in the interpreter and a lower bound natively).
                        let mtime = self
                            .virtual_times
                            .get(&path)
                            .copied()
                            .unwrap_or(VIRTUAL_MTIME_SECS);
                        self.write_fs_stat(arguments.get(1).copied(), frame, mode, size, mtime);
                        0
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            FilesystemHostOperation::ReadFileMetadata => {
                // `fstat(fd, buf)`: like `stat` but keyed by an OPEN descriptor. Map
                // the fd to its path, then fill the same stat record (a held `File`
                // is always a regular file here). EBADF for an unknown fd. Never
                // touches the cursor.
                let fd = self.eval_fs_fd(arguments.first().copied(), frame)?;
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
                        self.write_fs_stat(arguments.get(1).copied(), frame, mode, size, mtime);
                        0
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF (unknown descriptor)
                        -1
                    }
                }
            }
            FilesystemHostOperation::ReadSymlinkMetadata => {
                // `lstat(path, buf)`: like `stat`, but does NOT follow a final
                // symlink. A symlink reports S_IFLNK(0o120000)|0o777 with size =
                // the target path length (POSIX: a symlink's size is its target's
                // byte length); everything else is identical to `stat`.
                let path = self.eval_fs_bytes(arguments.first().copied(), frame)?;
                let meta = if let Some(target) = self.virtual_symlinks.get(&path) {
                    Some((0o120_000u16 | 0o777, target.len() as i64))
                } else {
                    let chmod_perm = self
                        .virtual_perms
                        .get(&path)
                        .map(|mode| (*mode as u16) & 0o7777);
                    if let Some(content) = self.virtual_files.get(&path) {
                        Some((
                            0o100_000u16 | chmod_perm.unwrap_or(0o644),
                            content.len() as i64,
                        ))
                    } else if self.virtual_dirs.contains(&path) {
                        Some((0o040_000u16 | chmod_perm.unwrap_or(0o755), 0i64))
                    } else {
                        None
                    }
                };
                match meta {
                    Some((mode, size)) => {
                        self.write_fs_stat(
                            arguments.get(1).copied(),
                            frame,
                            mode,
                            size,
                            VIRTUAL_MTIME_SECS,
                        );
                        0
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
        };
        Ok(Value::Int(result))
    }

    /// Evaluate an argument to an integer scalar (fd / flags / offset / count).
    pub(super) fn eval_fs_scalar(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
    ) -> EvalResult<i64> {
        let Some(argument) = argument else {
            return Ok(0);
        };
        Ok(self.eval_expression(argument, frame)?.as_int().unwrap_or(0))
    }

    /// Evaluate a raw byte count exactly once and reject values that could wrap
    /// at the host `usize` boundary or force an unbounded provider allocation.
    pub(super) fn eval_fs_transfer_count(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
    ) -> EvalResult<usize> {
        let raw = self.eval_fs_scalar(argument, frame)?;
        checked_filesystem_transfer_count(raw).map_err(|error| match error {
            FilesystemTransferCountError::NegativeOrUnrepresentable => Halt::Trap(
                "filesystem transfer count is negative or not host-representable".to_owned(),
            ),
            FilesystemTransferCountError::ExceedsEvaluatorLimit => Halt::Trap(format!(
                "filesystem transfer count exceeds evaluator limit of {MAX_FILESYSTEM_TRANSFER_BYTES} bytes"
            )),
        })
    }

    /// Evaluate an argument expected to be byte data (a path or a write payload).
    pub(super) fn eval_fs_bytes(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
    ) -> EvalResult<Vec<u8>> {
        let Some(argument) = argument else {
            return Ok(Vec::new());
        };
        match self.eval_expression(argument, frame)? {
            Value::Str(text) => Ok(text.borrow().clone()),
            // A byte array or a subslice view of one (`buffer` / `buffer[0..n]`):
            // each element cell holds a byte as an `Int`. This is the write-side
            // mirror of `write_fs_buffer`'s `Array` arm, and lets a caller write
            // a bounded prefix of a buffer (Rust `fs::copy`, `write` of a slice).
            Value::Array(cells) => {
                let mut bytes = Vec::with_capacity(cells.len());
                for cell in &cells {
                    bytes.push(cell.borrow().as_int().unwrap_or(0) as u8);
                }
                Ok(bytes)
            }
            // `&mut buffer` / `&buffer`: a reference to a caller field/local (e.g. a
            // `set_file_times` timespec buffer built in place). Deref to the array.
            Value::Ref(target) => {
                if let Value::Array(cells) = &*target.borrow() {
                    let mut bytes = Vec::with_capacity(cells.len());
                    for cell in cells {
                        bytes.push(cell.borrow().as_int().unwrap_or(0) as u8);
                    }
                    Ok(bytes)
                } else {
                    unsupported("filesystem call expected byte data behind a reference".to_owned())
                }
            }
            other => unsupported(format!("filesystem call expected byte data, got {other:?}")),
        }
    }

    /// Evaluate a `File` handle argument to its raw descriptor. The interpreter
    /// carries the fd directly (see the `Opened` construction), but a wrapping
    /// single-field struct is accepted defensively.
    pub(super) fn eval_fs_fd(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
    ) -> EvalResult<i32> {
        let Some(argument) = argument else {
            return trap("filesystem call missing file handle");
        };
        let value = self.eval_expression(argument, frame)?;
        let fd = match &value {
            Value::Struct { fields, .. } => {
                fields.get("fd").and_then(|cell| cell.borrow().as_int())
            }
            other => other.as_int(),
        };
        fd.map(|fd| fd as i32)
            .ok_or_else(|| Halt::Trap("filesystem call file handle is not an fd".to_owned()))
    }

    /// Copy read bytes into a caller `&mut [u8]` buffer (a text carrier or a byte
    /// array), truncated to the buffer's length. Best-effort: the outcome's
    /// `count` is authoritative; an unrecognized buffer shape is left untouched.
    pub(super) fn write_fs_buffer(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
        bytes: &[u8],
    ) {
        let Some(argument) = argument else {
            return;
        };
        let Ok(cell) = self.resolve_place(argument, frame) else {
            return;
        };
        let cell = self.deref_cell(cell);
        let shape = cell.borrow().clone();
        match shape {
            Value::Str(text) => {
                *text.borrow_mut() = bytes.to_vec();
            }
            Value::Array(cells) => {
                let count = bytes.len().min(cells.len());
                for (slot, byte) in cells.iter().zip(bytes.iter()).take(count) {
                    *slot.borrow_mut() = Value::Int(*byte as i64);
                }
            }
            _ => {}
        }
    }

    /// Read the current value of a `&mut i64` argument (the in/out `position`
    /// cursor of `read_dir`), 0 if unresolvable.
    pub(super) fn read_fs_position(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
    ) -> i64 {
        let Some(argument) = argument else {
            return 0;
        };
        let Ok(cell) = self.resolve_place(argument, frame) else {
            return 0;
        };
        let value = self.deref_cell(cell).borrow().as_int().unwrap_or(0);
        value
    }

    /// Write back a `&mut i64` argument (the in/out `position` cursor).
    pub(super) fn write_fs_position(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
        value: i64,
    ) {
        let Some(argument) = argument else {
            return;
        };
        let Ok(cell) = self.resolve_place(argument, frame) else {
            return;
        };
        *self.deref_cell(cell).borrow_mut() = Value::Int(value);
    }

    /// Build the packed darwin `dirent` records for a directory: `.` and `..`
    /// then each IMMEDIATE child (files in `virtual_files`, subdirs in
    /// `virtual_dirs` directly under `dir_path/`). Each record is
    /// `[d_ino(8) d_seekoff(8) d_reclen@16(u16) d_namlen@18(u16) d_type@20(u8)
    /// d_name@21(namlen) NUL pad]`, `d_reclen = round_up_8(25 + namlen)` — the
    /// exact layout `___getdirentries64` produces, so byte counts and a parser
    /// agree with native.
    /// Resolve `name` RELATIVE to the open directory `dirfd` to a full virtual
    /// path (`dirfd`'s path + "/" + name). Returns None if `dirfd` is not an open
    /// directory descriptor. The `*at` ops do their path-joining here -- in Rust,
    /// the way the OS does natively -- so the Omega layer never builds a path.
    fn virtual_at_path(&self, dirfd: i32, name: &[u8]) -> Option<Vec<u8>> {
        let dir = self
            .virtual_fds
            .get(&dirfd)
            .filter(|descriptor| descriptor.is_dir)
            .map(|descriptor| descriptor.path.clone())?;
        let mut full = dir;
        full.push(b'/');
        full.extend_from_slice(name);
        Some(full)
    }

    /// The find-enumeration twin of `build_dirent_records` (fs rung 3a): the
    /// same entry set (".", "..", then the immediate children of `dir_path`)
    /// as (name, is_dir) pairs for a `find_first` cursor snapshot.
    fn build_find_entries(&self, dir_path: &[u8]) -> std::collections::VecDeque<(Vec<u8>, bool)> {
        let mut entries: std::collections::VecDeque<(Vec<u8>, bool)> =
            std::collections::VecDeque::from([(b".".to_vec(), true), (b"..".to_vec(), true)]);
        let mut prefix = dir_path.to_vec();
        prefix.push(b'/');
        let immediate_child = |path: &[u8]| -> Option<Vec<u8>> {
            let rest = path.strip_prefix(prefix.as_slice())?;
            if rest.is_empty() || rest.contains(&b'/') {
                None
            } else {
                Some(rest.to_vec())
            }
        };
        for path in self.virtual_files.keys() {
            if let Some(name) = immediate_child(path) {
                entries.push_back((name, false));
            }
        }
        for path in &self.virtual_dirs {
            if let Some(name) = immediate_child(path) {
                entries.push_back((name, true));
            }
        }
        entries
    }

    /// Fill a caller find-data buffer (`&mut [u8]`, >= 320 bytes) the way
    /// `FindFirstFileA`/`FindNextFileA` write WIN32_FIND_DATAA: file
    /// attributes u32 little-endian at byte 0 (FILE_ATTRIBUTE_DIRECTORY 0x10 /
    /// FILE_ATTRIBUTE_NORMAL 0x80) and the NUL-terminated entry name at byte
    /// 44. Other fields are left zero.
    pub(super) fn write_find_data(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
        name: &[u8],
        is_dir: bool,
    ) {
        let mut record = vec![0u8; 320];
        let attributes: u32 = if is_dir { 0x10 } else { 0x80 };
        record[0..4].copy_from_slice(&attributes.to_le_bytes());
        let name_len = name.len().min(259);
        record[44..44 + name_len].copy_from_slice(&name[..name_len]);
        self.write_fs_buffer(argument, frame, &record);
    }

    fn build_dirent_records(&self, dir_path: &[u8]) -> Vec<u8> {
        let mut entries: Vec<(Vec<u8>, u8)> = vec![(b".".to_vec(), 4), (b"..".to_vec(), 4)];
        let mut prefix = dir_path.to_vec();
        prefix.push(b'/');
        let immediate_child = |path: &[u8]| -> Option<Vec<u8>> {
            let rest = path.strip_prefix(prefix.as_slice())?;
            if rest.is_empty() || rest.contains(&b'/') {
                None
            } else {
                Some(rest.to_vec())
            }
        };
        for path in self.virtual_files.keys() {
            if let Some(name) = immediate_child(path) {
                entries.push((name, 8)); // DT_REG
            }
        }
        for path in &self.virtual_dirs {
            if let Some(name) = immediate_child(path) {
                entries.push((name, 4)); // DT_DIR
            }
        }
        pack_dirent_records(&entries)
    }

    /// Fill a caller stat buffer (`&mut [u8]` of at least 144 bytes) the way the
    /// darwin kernel writes `struct stat`: `st_mode` (u16) at byte offset 4 and
    /// `st_size` (i64) at byte offset 96, both little-endian. The Omega layer
    /// reads those fields back with byte-assembly. Other fields are left zero.
    pub(super) fn write_fs_stat(
        &mut self,
        argument: Option<ExpressionHandle>,
        frame: &Frame,
        mode: u16,
        size: i64,
        mtime_secs: i64,
    ) {
        let Some(argument) = argument else {
            return;
        };
        let Ok(cell) = self.resolve_place(argument, frame) else {
            return;
        };
        let cell = self.deref_cell(cell);
        if let Value::Array(cells) = &*cell.borrow() {
            let put = |offset: usize, byte: u8| {
                if let Some(slot) = cells.get(offset) {
                    *slot.borrow_mut() = Value::Int(i64::from(byte));
                }
            };
            // Lay the fields out at the HOST target's stat offsets (mirrors the
            // selected StatLayout policy the wrapper projects). On
            // windows the width-mismatched/absent fields go to a synthetic tail; a
            // real native `_stat64` would leave that tail zero.
            use host_stat_offsets as off;
            put(off::MODE, (mode & 0xff) as u8);
            put(off::MODE + 1, (mode >> 8) as u8);
            // st_nlink: the hermetic FS models a fixed link count of 1 -- it does not
            // track hard-link groups (its `hard_link` copies bytes), so every path
            // reports 1. Native `stat` returns the real count (2 after a `hard_link`);
            // that case is asserted only in the native canary.
            put(off::NLINK, 1);
            put(off::NLINK + 1, 0);
            for i in 0..8 {
                put(off::INO + i, (VIRTUAL_INO >> (8 * i)) as u8);
                put(off::ATIME + i, (VIRTUAL_ATIME_SECS >> (8 * i)) as u8);
                put(off::MTIME + i, (mtime_secs >> (8 * i)) as u8);
                put(off::CTIME + i, (VIRTUAL_CTIME_SECS >> (8 * i)) as u8);
                put(off::BTIME + i, (VIRTUAL_BIRTHTIME_SECS >> (8 * i)) as u8);
                put(off::SIZE + i, (size >> (8 * i)) as u8);
                put(off::BLOCKS + i, (VIRTUAL_BLOCKS >> (8 * i)) as u8);
            }
            for i in 0..4 {
                put(off::DEV + i, (VIRTUAL_DEV >> (8 * i)) as u8);
                put(off::UID + i, (VIRTUAL_UID >> (8 * i)) as u8);
                put(off::GID + i, (VIRTUAL_GID >> (8 * i)) as u8);
                put(off::BLKSIZE + i, (VIRTUAL_BLKSIZE >> (8 * i)) as u8);
            }
        }
    }

    /// Mint a fresh descriptor over `path`; `create` truncates (or creates) the
    /// file first.
    fn virtual_open(&mut self, path: Vec<u8>, writable: bool, create: bool) -> i32 {
        if create {
            self.virtual_files.insert(path.clone(), Vec::new());
        }
        let fd = self.virtual_next_fd;
        self.virtual_next_fd += 1;
        self.virtual_fds.insert(
            fd,
            VirtualFd {
                path,
                cursor: 0,
                writable,
                is_dir: false,
            },
        );
        fd
    }

    /// Write `bytes` at the descriptor's cursor (extending the file as needed),
    /// advancing the cursor. `None` if the fd is unknown or not writable.
    fn virtual_write(&mut self, fd: i32, bytes: &[u8]) -> Option<usize> {
        let descriptor = self.virtual_fds.get(&fd)?;
        if !descriptor.writable {
            return None;
        }
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor;
        let content = self.virtual_files.get_mut(&path)?;
        let end = cursor + bytes.len();
        if content.len() < end {
            content.resize(end, 0);
        }
        content[cursor..end].copy_from_slice(bytes);
        if let Some(descriptor) = self.virtual_fds.get_mut(&fd) {
            descriptor.cursor = end;
        }
        Some(bytes.len())
    }

    /// Read up to `count` bytes from the descriptor's cursor, advancing it.
    /// `None` if the fd is unknown.
    fn virtual_read_n(&mut self, fd: i32, count: usize) -> Option<Vec<u8>> {
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor;
        let content = self.virtual_files.get(&path)?;
        let available = content.get(cursor..).unwrap_or(&[]);
        let take = available.len().min(count);
        let bytes = available[..take].to_vec();
        if let Some(descriptor) = self.virtual_fds.get_mut(&fd) {
            descriptor.cursor = cursor + take;
        }
        Some(bytes)
    }

    /// Read up to `count` bytes starting at absolute `offset` WITHOUT moving the
    /// cursor (Rust `FileExt::read_at` / `pread`). `None` if the fd is unknown or
    /// the offset is negative. A read past end-of-file yields fewer (or zero) bytes.
    fn virtual_read_at(&mut self, fd: i32, offset: i64, count: usize) -> Option<Vec<u8>> {
        if offset < 0 {
            return None;
        }
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let content = self.virtual_files.get(&path)?;
        let available = content.get(offset as usize..).unwrap_or(&[]);
        let take = available.len().min(count);
        Some(available[..take].to_vec())
    }

    /// Write `bytes` at absolute `offset` (extending + zero-filling any gap) WITHOUT
    /// moving the cursor (Rust `FileExt::write_at` / `pwrite`). `None` if the fd is
    /// unknown, not writable, or the offset is negative.
    fn virtual_write_at(&mut self, fd: i32, offset: i64, bytes: &[u8]) -> Option<usize> {
        if offset < 0 {
            return None;
        }
        let descriptor = self.virtual_fds.get(&fd)?;
        if !descriptor.writable {
            return None;
        }
        let path = descriptor.path.clone();
        let start = offset as usize;
        let content = self.virtual_files.get_mut(&path)?;
        let end = start + bytes.len();
        if content.len() < end {
            content.resize(end, 0);
        }
        content[start..end].copy_from_slice(bytes);
        Some(bytes.len())
    }

    /// `open(path, flags)`: model the O_CREAT/O_TRUNC/O_APPEND/access bits.
    /// Returns a fresh fd, or -1 if the path is absent and O_CREAT is not set.
    fn virtual_open_flags(&mut self, path: Vec<u8>, flags: i32) -> i32 {
        // Follow one symlink level (the canonicalize/read_link model): native
        // open on BOTH families resolves symlinks, and the hermetic open never
        // did -- surfaced when the windows canonicalize composition made open
        // its entry point. The descriptor stores the RESOLVED path, so
        // handle-keyed consumers (final_path_name_by_handle) report the final
        // target exactly like Win32.
        let path = self.virtual_symlinks.get(&path).cloned().unwrap_or(path);
        let exists = self.virtual_files.contains_key(&path);
        let o_creat = host_open_flags::o_creat(flags);
        let o_trunc = host_open_flags::o_trunc(flags);
        let o_append = host_open_flags::o_append(flags);
        let writable = flags & 0x3 != 0; // O_WRONLY | O_RDWR (universal)
        // Opening a directory for writing is EISDIR (Rust `ErrorKind::IsADirectory`).
        // Checked before the ENOENT test so a dir path (never in `virtual_files`)
        // reports the more specific kind.
        if self.virtual_dirs.contains(&path) && writable {
            self.virtual_errno = 21; // EISDIR
            return -1;
        }
        // Permission enforcement: opening a chmod'd path fails with EACCES when
        // the needed bit is clear — the owner-write bit (0o200) for a write-open,
        // or the owner-read bit (0o400) for a read-open (Rust
        // `ErrorKind::PermissionDenied`).
        let needed_bit = if writable { 0o200 } else { 0o400 };
        if self
            .virtual_perms
            .get(&path)
            .is_some_and(|mode| mode & needed_bit == 0)
        {
            self.virtual_errno = 13; // EACCES
            return -1;
        }
        // Read-open of a DIRECTORY: POSIX allows opening a dir read-only (the
        // basis for `read_dir`). Mint a dir descriptor. Checked before the ENOENT
        // test since a dir path is never in `virtual_files`. (This also aligns
        // `exists`/`try_exists` on a dir with native, where opening a dir works.)
        if !writable && self.virtual_dirs.contains(&path) {
            let fd = self.virtual_next_fd;
            self.virtual_next_fd += 1;
            self.virtual_fds.insert(
                fd,
                VirtualFd {
                    path,
                    cursor: 0,
                    writable: false,
                    is_dir: true,
                },
            );
            return fd;
        }
        if !exists && !o_creat {
            self.virtual_errno = 2; // ENOENT
            return -1;
        }
        if !exists || o_trunc {
            self.virtual_files.insert(path.clone(), Vec::new());
        }
        let cursor = if o_append {
            self.virtual_files.get(&path).map_or(0, Vec::len)
        } else {
            0
        };
        let fd = self.virtual_next_fd;
        self.virtual_next_fd += 1;
        self.virtual_fds.insert(
            fd,
            VirtualFd {
                path,
                cursor,
                writable,
                is_dir: false,
            },
        );
        fd
    }

    /// `lseek(fd, offset, whence)`: reposition the cursor, returning the new
    /// absolute offset. `None` on unknown fd, bad whence, or a negative result.
    fn virtual_seek(&mut self, fd: i32, offset: i64, whence: i32) -> Option<i64> {
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor as i64;
        let len = self.virtual_files.get(&path).map_or(0, Vec::len) as i64;
        let new_pos = match whence {
            0 => offset,          // SEEK_SET
            1 => cursor + offset, // SEEK_CUR
            2 => len + offset,    // SEEK_END
            _ => return None,
        };
        if new_pos < 0 {
            return None;
        }
        if let Some(descriptor) = self.virtual_fds.get_mut(&fd) {
            descriptor.cursor = new_pos as usize;
        }
        Some(new_pos)
    }

    /// `ftruncate(fd, length)`: resize the file backing `fd` (truncate or
    /// zero-extend). Returns 0 on success, -1 on an unknown fd/path.
    fn virtual_set_len(&mut self, fd: i32, length: i64) -> i64 {
        let Some(descriptor) = self.virtual_fds.get(&fd) else {
            return -1;
        };
        let path = descriptor.path.clone();
        let Some(content) = self.virtual_files.get_mut(&path) else {
            return -1;
        };
        content.resize(length.max(0) as usize, 0);
        0
    }

    /// The non-root `chown`/`fchown`/`lchown` rule shared by the ownership
    /// handlers: a change to the CURRENT owner -- or a uid/gid of -1, meaning
    /// "leave that component unchanged" -- is a permitted no-op (returns 0); any
    /// OTHER owner is EPERM (sets errno 1, returns -1). Mirrors what the native
    /// syscalls do for a normal (non-root) user, keeping the two engines'
    /// differential consistent.
    fn virtual_chown_result(&mut self, uid: i32, gid: i32) -> i64 {
        let effective_uid = if uid == -1 { VIRTUAL_UID as i32 } else { uid };
        let effective_gid = if gid == -1 { VIRTUAL_GID as i32 } else { gid };
        if effective_uid == VIRTUAL_UID as i32 && effective_gid == VIRTUAL_GID as i32 {
            0
        } else {
            self.virtual_errno = 1; // EPERM
            -1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FilesystemTransferCountError, MAX_FILESYSTEM_TRANSFER_BYTES,
        checked_filesystem_transfer_count,
    };

    #[test]
    fn transfer_count_rejects_wrap_and_unbounded_allocation() {
        assert_eq!(
            checked_filesystem_transfer_count(-1),
            Err(FilesystemTransferCountError::NegativeOrUnrepresentable)
        );
        assert_eq!(
            checked_filesystem_transfer_count(MAX_FILESYSTEM_TRANSFER_BYTES as i64 + 1),
            Err(FilesystemTransferCountError::ExceedsEvaluatorLimit)
        );
    }

    #[test]
    fn transfer_count_accepts_the_closed_interval_through_the_limit() {
        assert_eq!(checked_filesystem_transfer_count(0), Ok(0));
        assert_eq!(
            checked_filesystem_transfer_count(MAX_FILESYSTEM_TRANSFER_BYTES as i64),
            Ok(MAX_FILESYSTEM_TRANSFER_BYTES)
        );
    }
}
