//! Operations addressed by path: opens and creates, removals, renames,
//! links, permissions, ownership, canonicalization and path metadata.

use crate::FilesystemMetadataObservationKind;
use crate::interpreter::evaluator::{
    EvalResult, PreparedFilesystemCall, VIRTUAL_MTIME_SECS, host_open_flags,
};

impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
    pub(super) fn serve_open_path_handle(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::OpenPathHandle {
            path,
            desired_access: _,
            share_mode: _,
            security_attributes: _,
            creation_disposition: _,
            flags_and_attributes: _,
            template_file: _,
        } = call
        else {
            unreachable!("dispatched serve_open_path_handle")
        };
        Ok({
            // Hermetic CreateFileA model for metadata/query handles. The
            // wrapper supplies access=0 + OPEN_EXISTING; the virtual fd
            // table already models both files and read-only directories.
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
        })
    }

    pub(super) fn serve_open_create(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::OpenCreate { path, flags, mode } = call else {
            unreachable!("dispatched serve_open_create")
        };
        Ok({
            // `open(path, flags, mode)` with O_CREAT (Rust `File::create_new`,
            // `OpenOptions.create`/`.create_new`). Flag bits are the HOST's
            // (host_open_flags, mirroring the checked target encoder). This
            // adds the O_EXCL/EEXIST atomic
            // create-new guard + create-mode recording; every other flag bit
            // (O_TRUNC/O_APPEND/access/EACCES/ENOENT) is handled by the shared
            // `virtual_open_flags`, so `open_create` cleanly SUBSUMES `open`.
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
                    self.virtual_perms.insert(path, (mode as u32) & 0o777);
                }
                fd as i64
            }
        })
    }

    pub(super) fn serve_remove(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let (PreparedFilesystemCall::Remove { path } | PreparedFilesystemCall::RemoveName { path }) =
            call
        else {
            unreachable!("dispatched serve_remove")
        };
        Ok({
            if self.virtual_files.remove(&path).is_some() {
                0
            } else {
                self.virtual_errno = 2; // ENOENT
                -1
            }
        })
    }

    pub(super) fn serve_create_dir(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let (PreparedFilesystemCall::CreateDir { path, mode: _ }
        | PreparedFilesystemCall::CreateDirName {
            name: path,
            mode: _,
        }) = call
        else {
            unreachable!("dispatched serve_create_dir")
        };
        Ok({
            // -1 (EEXIST) if the dir already exists.
            if self.virtual_dirs.insert(path) {
                0
            } else {
                self.virtual_errno = 17; // EEXIST
                -1
            }
        })
    }

    pub(super) fn serve_remove_dir(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let (PreparedFilesystemCall::RemoveDir { path }
        | PreparedFilesystemCall::RemoveDirName { path }) = call
        else {
            unreachable!("dispatched serve_remove_dir")
        };
        Ok({
            if self.virtual_dirs.remove(&path) {
                0
            } else {
                self.virtual_errno = 2; // ENOENT
                -1
            }
        })
    }

    pub(super) fn serve_open_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::OpenAt { dirfd, name, flags } = call else {
            unreachable!("dispatched serve_open_at")
        };
        Ok({
            // `openat(dirfd, name, flags)`: open `name` relative to the open
            // directory `dirfd`. The full path (dirfd's path + "/" + name) is
            // joined HERE (the OS does it natively), so no Omega path build.
            match self.virtual_at_path(dirfd, &name) {
                Some(full) => self.virtual_open_flags(full, flags) as i64,
                None => {
                    self.virtual_errno = 9; // EBADF (dirfd not an open directory)
                    -1
                }
            }
        })
    }

    pub(super) fn serve_unlink_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::UnlinkAt { dirfd, name, flags } = call else {
            unreachable!("dispatched serve_unlink_at")
        };
        Ok({
            // `unlinkat(dirfd, name, flags)`: remove `name` relative to `dirfd`.
            // flags & AT_REMOVEDIR(0x80) removes an empty directory, else a file.
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
        })
    }

    pub(super) fn serve_set_permissions(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::SetPermissions { path, mode } = call else {
            unreachable!("dispatched serve_set_permissions")
        };
        Ok({
            // `chmod(path, mode)`: record the mode. ENOENT if the path names
            // neither a file nor a directory. `mode` is the second arg.
            if self.virtual_files.contains_key(&path) || self.virtual_dirs.contains(&path) {
                self.virtual_perms.insert(path, mode);
                0
            } else {
                self.virtual_errno = 2; // ENOENT
                -1
            }
        })
    }

    pub(super) fn serve_change_owner(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let (PreparedFilesystemCall::ChangeOwner { path, uid, gid }
        | PreparedFilesystemCall::ChangeOwnerNoFollow { path, uid, gid }) = call
        else {
            unreachable!("dispatched serve_change_owner")
        };
        Ok({
            // `chown`/`lchown(path, uid, gid)`: change owner/group. ENOENT if
            // the path is absent. The hermetic model's process identity is
            // VIRTUAL_UID/GID (a normal, non-root user), so only a NO-OP change
            // is permitted: a uid/gid of -1 leaves that component alone, and
            // setting the CURRENT owner succeeds; any OTHER owner is EPERM --
            // exactly what native `chown` does when run as a normal user.
            // (`lchown` differs from `chown` only on symlinks, which the
            // hermetic FS never follows on ownership ops, so they behave
            // identically here.)
            let exists = self.virtual_files.contains_key(&path)
                || self.virtual_dirs.contains(&path)
                || self.virtual_symlinks.contains_key(&path);
            if !exists {
                self.virtual_errno = 2; // ENOENT
                -1
            } else {
                self.virtual_chown_result(uid, gid)
            }
        })
    }

    pub(super) fn serve_rename(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Rename { from, to } = call else {
            unreachable!("dispatched serve_rename")
        };
        Ok({
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
        })
    }

    pub(super) fn serve_hard_link(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::HardLink { original, link } = call else {
            unreachable!("dispatched serve_hard_link")
        };
        Ok({
            // `link(original, link)`: a second name for the same inode.
            // ENOENT if the original is absent; EEXIST if the link name is
            // taken. The hermetic FS has no inodes, so this snapshots the
            // modeled file state. A later mutation still will not propagate
            // between names, but the snapshot is exact when neither name is
            // subsequently mutated.
            if self.virtual_files.contains_key(&link)
                || self.virtual_dirs.contains(&link)
                || self.virtual_symlinks.contains_key(&link)
            {
                self.virtual_errno = 17; // EEXIST
                -1
            } else if self.virtual_hard_link_file(&original, link) {
                0
            } else {
                self.virtual_errno = 2; // ENOENT
                -1
            }
        })
    }

    pub(super) fn serve_create_hard_link(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::CreateHardLink {
            link,
            existing,
            security_attributes: _,
        } = call
        else {
            unreachable!("dispatched serve_create_hard_link")
        };
        Ok({
            // `CreateHardLinkA(link, existing, security)` -- the WINDOWS
            // hard-link primitive (session slice 3): the ARG ORDER is
            // (new link, existing), REVERSED from `hard_link`, and the
            // result is BOOL (1 success / 0 failure). Same hermetic
            // snapshot model as `hard_link` above. virtual_errno is
            // also the provider's Win32 last-error slot for GetLastError.
            if self.virtual_files.contains_key(&link)
                || self.virtual_dirs.contains(&link)
                || self.virtual_symlinks.contains_key(&link)
            {
                self.virtual_errno = 183; // ERROR_ALREADY_EXISTS
                0
            } else if self.virtual_hard_link_file(&existing, link) {
                1
            } else {
                self.virtual_errno = 2; // ERROR_FILE_NOT_FOUND
                0
            }
        })
    }

    pub(super) fn serve_symlink(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Symlink { target, link } = call else {
            unreachable!("dispatched serve_symlink")
        };
        Ok({
            // `symlink(target, linkpath)`: record the link -> target mapping.
            // EEXIST if the link name already names a file/dir/symlink.
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
        })
    }

    pub(super) fn serve_read_link(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::ReadLink {
            path,
            buffer,
            count,
        } = call
        else {
            unreachable!("dispatched serve_read_link")
        };
        Ok({
            // `readlink(path, buf, count)`: write the target bytes into the
            // buffer (up to `count`), returning the number written. ENOENT if
            // `path` is not a symlink in the hermetic model.
            match self.virtual_symlinks.get(&path).cloned() {
                Some(target) => {
                    let n = target.len().min(count.host);
                    buffer.write(&target[..n])?;
                    n as i64
                }
                None => {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
        })
    }

    pub(super) fn serve_canonicalize(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Canonicalize { path, buffer } = call else {
            unreachable!("dispatched serve_canonicalize")
        };
        Ok({
            // `realpath(path, buf)`: resolve `path` to its canonical absolute
            // form and write it NUL-terminated into the buffer. The hermetic FS
            // is already absolute and does not resolve `.`/`..`; it follows one
            // symlink level (matching `read_link`). Returns a non-zero success
            // flag (native returns the resolved-buffer pointer) or 0 (NULL) +
            // ENOENT when the target does not exist.
            let resolved = self.virtual_symlinks.get(&path).cloned().unwrap_or(path);
            let exists =
                self.virtual_files.contains_key(&resolved) || self.virtual_dirs.contains(&resolved);
            if exists {
                let mut bytes = resolved.clone();
                bytes.push(0); // NUL-terminate like realpath's C string
                buffer.write(&bytes)?;
                1
            } else {
                self.virtual_errno = 2; // ENOENT
                0
            }
        })
    }

    pub(super) fn serve_read_metadata(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::ReadMetadata { path, buffer } = call else {
            unreachable!("dispatched serve_read_metadata")
        };
        Ok({
            // `stat(path, buf)`: fill the buffer's st_mode (off 4, u16) and
            // st_size (off 96, i64) as the darwin kernel would. A regular
            // file is S_IFREG(0o100000)|0o644 with size = content length; a
            // directory is S_IFDIR(0o040000)|0o755 size 0. ENOENT otherwise.
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
                    self.write_fs_stat(
                        &buffer,
                        FilesystemMetadataObservationKind::FollowedPath,
                        u32::from(mode),
                        size,
                        mtime,
                    )?;
                    0
                }
                None => {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
        })
    }

    pub(super) fn serve_read_symlink_metadata(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::ReadSymlinkMetadata { path, buffer } = call else {
            unreachable!("dispatched serve_read_symlink_metadata")
        };
        Ok({
            // `lstat(path, buf)`: like `stat`, but does NOT follow a final
            // symlink. A symlink reports S_IFLNK(0o120000)|0o777 with size =
            // the target path length (POSIX: a symlink's size is its target's
            // byte length); everything else is identical to `stat`.
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
                        &buffer,
                        FilesystemMetadataObservationKind::UnfollowedFinalPath,
                        u32::from(mode),
                        size,
                        VIRTUAL_MTIME_SECS,
                    )?;
                    0
                }
                None => {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
        })
    }
}
