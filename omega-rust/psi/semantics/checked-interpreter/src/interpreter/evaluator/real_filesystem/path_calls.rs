//! Operations addressed by path: opens and creates, removals, renames,
//! links, permissions, ownership, canonicalization and path metadata.

use super::super::{
    EvalResult, FilesystemGrantRefusalReason, PreparedFilesystemCall, host_open_flags,
};
use super::{
    EACCES, EBADF, EINVAL, ENOENT, io_errno, open_options_for, open_real, real_os_bytes, real_path,
};

impl<'program> super::super::Evaluator<'program> {
    pub(super) fn real_create(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Create { path, mode: _ } = call else {
            unreachable!("dispatched real_create")
        };
        Ok({
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
        })
    }

    pub(super) fn real_open_create(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::OpenCreate { path, flags, mode } = call else {
            unreachable!("dispatched real_open_create")
        };
        Ok({
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
        })
    }

    pub(super) fn real_open_path_handle(
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
            unreachable!("dispatched real_open_path_handle")
        };
        Ok({
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
        })
    }

    pub(super) fn real_remove(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let (PreparedFilesystemCall::Remove { path } | PreparedFilesystemCall::RemoveName { path }) =
            call
        else {
            unreachable!("dispatched real_remove")
        };
        Ok({
            match self.authorized_namespace_leaf(&path, true, 0) {
                Some(path) => {
                    let prepared = self.prepare_sponsored_unlink(&path)?;
                    let outcome = std::fs::remove_file(path);
                    self.finish_real_mutation(outcome, prepared, false)?
                }
                None => -1,
            }
        })
    }

    pub(super) fn real_remove_dir(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let (PreparedFilesystemCall::RemoveDir { path }
        | PreparedFilesystemCall::RemoveDirName { path }) = call
        else {
            unreachable!("dispatched real_remove_dir")
        };
        Ok({
            match self.authorized_namespace_leaf(&path, true, 0) {
                Some(path) => {
                    let prepared = self.prepare_sponsored_unlink(&path)?;
                    let outcome = std::fs::remove_dir(path);
                    self.finish_real_mutation(outcome, prepared, false)?
                }
                None => -1,
            }
        })
    }

    pub(super) fn real_rename(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Rename { from, to } = call else {
            unreachable!("dispatched real_rename")
        };
        Ok({
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
        })
    }

    pub(super) fn real_canonicalize(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Canonicalize { path, buffer } = call else {
            unreachable!("dispatched real_canonicalize")
        };
        Ok({
            // `realpath(path, buf)`: NUL-terminated resolved path into the
            // buffer; non-zero success flag, 0 (NULL) + errno on failure --
            // the virtual contract's shape.
            match self.authorized_path(&path, false, 0) {
                Some(path) => match std::fs::canonicalize(&path) {
                    Ok(resolved) => {
                        let Some(path_bytes) = real_os_bytes(resolved.as_os_str()) else {
                            self.real_fs_mut().errno = EINVAL;
                            return Ok(0);
                        };
                        let mut bytes = path_bytes.clone();
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
        })
    }

    pub(super) fn real_hard_link(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::HardLink { original, link } = call else {
            unreachable!("dispatched real_hard_link")
        };
        Ok({
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
        })
    }

    pub(super) fn real_create_hard_link(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::CreateHardLink {
            link,
            existing,
            security_attributes: _,
        } = call
        else {
            unreachable!("dispatched real_create_hard_link")
        };
        Ok({
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
        })
    }

    pub(super) fn real_symlink(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Symlink { target, link } = call else {
            unreachable!("dispatched real_symlink")
        };
        Ok({
            // `symlink(target, linkpath)`: the TARGET is stored verbatim
            // (never dereferenced here), so only the link path needs write
            // authority. Unix-only in std; elsewhere super::ENOTSUP.
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
                        self.real_fs_mut().errno = super::ENOTSUP;
                        -1
                    }
                }
                None => -1,
            }
        })
    }

    pub(super) fn real_read_link(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::ReadLink {
            path,
            buffer,
            count,
        } = call
        else {
            unreachable!("dispatched real_read_link")
        };
        Ok({
            // `readlink(path, buf, count)`: target bytes into the buffer,
            // returns the count written.
            match self.authorized_path_no_follow(&path, false, 0) {
                Some(path) => match std::fs::read_link(&path) {
                    Ok(target) => {
                        let Some(bytes) = real_os_bytes(target.as_os_str()) else {
                            self.real_fs_mut().errno = EINVAL;
                            return Ok(-1);
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
        })
    }

    pub(super) fn real_set_permissions(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::SetPermissions { path, mode } = call else {
            unreachable!("dispatched real_set_permissions")
        };
        Ok({
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
                        self.real_fs_mut().errno = super::ENOTSUP;
                        -1
                    }
                }
                None => -1,
            }
        })
    }

    pub(super) fn real_change_owner(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::ChangeOwner { path, uid, gid } = call else {
            unreachable!("dispatched real_change_owner")
        };
        Ok({
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
                        self.real_fs_mut().errno = super::ENOTSUP;
                        -1
                    }
                }
                None => -1,
            }
        })
    }

    pub(super) fn real_unlink_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::UnlinkAt { dirfd, name, flags } = call else {
            unreachable!("dispatched real_unlink_at")
        };
        Ok({
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
                        return Ok(-1);
                    }
                },
                None => {
                    self.real_fs_mut().errno = EBADF;
                    return Ok(-1);
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
        })
    }

    pub(super) fn real_open_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::OpenAt { dirfd, name, flags } = call else {
            unreachable!("dispatched real_open_at")
        };
        Ok({
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
                        return Ok(-1);
                    }
                },
                None => {
                    self.real_fs_mut().errno = EBADF;
                    return Ok(-1);
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
        })
    }
}
