//! Operations on an open descriptor or handle: reads, writes, seeks,
//! duplication, extents, syncs, locks, and descriptor-scoped metadata.

use super::super::{EvalResult, PreparedFilesystemCall, synthetic_handle_fd};
use super::{
    EBADF, SelectedFilesystemMetadata, checked_written_count, io_errno, positioned_read,
    positioned_write, real_lock, real_lock_win32, real_os_bytes, win32_error_code,
};
use crate::FilesystemMetadataObservationKind;
use std::io::{Read, Seek, SeekFrom, Write};

impl<'program> super::super::Evaluator<'program> {
    pub(super) fn real_read(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Read { fd, buffer, count } = call else {
            unreachable!("dispatched real_read")
        };
        Ok({
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
        })
    }

    pub(super) fn real_write(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Write { fd, bytes } = call else {
            unreachable!("dispatched real_write")
        };
        Ok({
            if !self.require_real_descriptor_write_grant(fd, false) {
                return Ok(-1);
            }
            let prepared = match self.prepare_sponsored_write(fd, bytes.len(), None)? {
                Ok(prepared) => prepared,
                Err(errno) => {
                    self.real_fs_mut().errno = errno;
                    return Ok(-1);
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
        })
    }

    pub(super) fn real_seek(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Seek { fd, offset, whence } = call else {
            unreachable!("dispatched real_seek")
        };
        Ok({
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
        })
    }

    pub(super) fn real_close(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Close { fd } = call else {
            unreachable!("dispatched real_close")
        };
        Ok({
            if self.real_fs_mut().files.contains_key(&fd) {
                let prepared = self.prepare_sponsored_close(fd)?;
                self.real_fs_mut().files.remove(&fd);
                self.commit_sponsored_mutation(prepared)?;
                0 // the File drop closes the real descriptor
            } else {
                self.real_fs_mut().errno = EBADF;
                -1
            }
        })
    }

    pub(super) fn real_close_handle(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::CloseHandle { handle } = call else {
            unreachable!("dispatched real_close_handle")
        };
        Ok({
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
        })
    }

    pub(super) fn real_duplicate(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::Duplicate { fd } = call else {
            unreachable!("dispatched real_duplicate")
        };
        Ok({
            let prepared = self.prepare_sponsored_duplicate(fd)?;
            let cloned = match self.real_fs_mut().files.get(&fd) {
                Some(entry) => entry
                    .file
                    .try_clone()
                    .map(|file| {
                        (
                            file,
                            entry.path.clone(),
                            entry.append,
                            entry.canonical_metadata,
                        )
                    })
                    .map_err(|error| io_errno(&error)),
                None => Err(EBADF),
            };
            match cloned {
                Ok((file, path, append, canonical_metadata)) => {
                    let sponsor_descriptor = self.commit_sponsored_open(prepared)?;
                    self.real_fs_mut().insert_preselected(
                        file,
                        path,
                        sponsor_descriptor,
                        append,
                        canonical_metadata,
                    )
                }
                Err(errno) => {
                    self.real_fs_mut().errno = errno;
                    -1
                }
            }
        })
    }

    pub(super) fn real_set_len(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::SetLen { fd, length } = call else {
            unreachable!("dispatched real_set_len")
        };
        Ok({
            if !self.require_real_descriptor_write_grant(fd, false) {
                return Ok(-1);
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
        })
    }

    pub(super) fn real_sync(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let (PreparedFilesystemCall::Sync { fd } | PreparedFilesystemCall::SyncData { fd }) = call
        else {
            unreachable!("dispatched real_sync")
        };
        Ok({
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
        })
    }

    pub(super) fn real_read_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::ReadAt {
            fd,
            buffer,
            count,
            offset,
        } = call
        else {
            unreachable!("dispatched real_read_at")
        };
        Ok({
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
        })
    }

    pub(super) fn real_write_at(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::WriteAt { fd, bytes, offset } = call else {
            unreachable!("dispatched real_write_at")
        };
        Ok({
            // `pwrite(fd, buf, offset)`: write at an absolute offset
            // WITHOUT moving the cursor (same emulation).
            if !self.require_real_descriptor_write_grant(fd, false) {
                return Ok(-1);
            }
            let prepared = match self.prepare_sponsored_write(fd, bytes.len(), Some(offset))? {
                Ok(prepared) => prepared,
                Err(errno) => {
                    self.real_fs_mut().errno = errno;
                    return Ok(-1);
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
        })
    }

    pub(super) fn real_read_file_metadata(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::ReadFileMetadata { fd, buffer } = call else {
            unreachable!("dispatched real_read_file_metadata")
        };
        Ok({
            let looked_up = match self.real_fs_mut().files.get(&fd) {
                Some(entry) => match entry.canonical_metadata {
                    Some(metadata) => Ok(SelectedFilesystemMetadata::Canonical(metadata)),
                    None => entry
                        .file
                        .metadata()
                        .map(SelectedFilesystemMetadata::Physical)
                        .map_err(|error| io_errno(&error)),
                },
                None => Err(EBADF),
            };
            match looked_up {
                Ok(SelectedFilesystemMetadata::Canonical(metadata)) => {
                    self.write_canonical_fs_stat(
                        &buffer,
                        FilesystemMetadataObservationKind::OpenDescriptor,
                        metadata,
                    )?;
                    0
                }
                Ok(SelectedFilesystemMetadata::Physical(metadata)) => {
                    self.write_real_fs_stat(
                        &buffer,
                        FilesystemMetadataObservationKind::OpenDescriptor,
                        &metadata,
                    )?;
                    0
                }
                Err(errno) => {
                    self.real_fs_mut().errno = errno;
                    -1
                }
            }
        })
    }

    pub(super) fn real_get_osf_handle(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::GetOsfHandle { fd } = call else {
            unreachable!("dispatched real_get_osf_handle")
        };
        Ok({
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
        })
    }

    pub(super) fn real_final_path_name_by_handle(
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
            unreachable!("dispatched real_final_path_name_by_handle")
        };
        Ok({
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
                            return Ok(0);
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
        })
    }

    pub(super) fn real_set_file_time(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::SetFileTime {
            handle,
            creation: _,
            last_access: _,
            last_write: write_ft,
        } = call
        else {
            unreachable!("dispatched real_set_file_time")
        };
        Ok({
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
                return Ok(0);
            };
            if !self.require_real_descriptor_write_grant(handle, true) {
                return Ok(0);
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
        })
    }

    pub(super) fn real_set_file_permissions(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::SetFilePermissions { fd, mode } = call else {
            unreachable!("dispatched real_set_file_permissions")
        };
        Ok({
            // `fchmod(fd, mode)`: the descriptor must retain write grant
            // even when the host permits metadata changes on a read-only
            // open file description.
            if !self.require_real_descriptor_write_grant(fd, false) {
                return Ok(-1);
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
                        real.errno = super::ENOTSUP;
                        -1
                    }
                }
                None => {
                    real.errno = EBADF;
                    -1
                }
            }
        })
    }

    pub(super) fn real_set_file_times(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::SetFileTimes { fd, times } = call else {
            unreachable!("dispatched real_set_file_times")
        };
        Ok({
            // `futimens(fd, times)`: two packed timespecs (atime, mtime);
            // the model (virtual and real alike) applies the MODIFIED time
            // -- times[1].tv_sec at byte offset 16.
            let mtime_secs = times
                .bytes
                .get(16..24)
                .map(|bytes| i64::from_le_bytes(bytes.try_into().unwrap()))
                .unwrap_or(0);
            if !self.require_real_descriptor_write_grant(fd, false) {
                return Ok(-1);
            }
            let real = self.real_fs_mut();
            match real.files.get_mut(&fd) {
                Some(entry) => {
                    let stamp = if mtime_secs >= 0 {
                        std::time::UNIX_EPOCH + std::time::Duration::from_secs(mtime_secs as u64)
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
        })
    }

    pub(super) fn real_lock_file(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::LockFile { fd, operation } = call else {
            unreachable!("dispatched real_lock_file")
        };
        Ok({
            // `flock(fd, op)`: LOCK_SH=1 LOCK_EX=2 LOCK_NB=4 LOCK_UN=8,
            // served by std's advisory file locks on the real handle.
            if !self.require_real_descriptor_write_grant(fd, false) {
                return Ok(-1);
            }
            let real = self.real_fs_mut();
            match real.files.get(&fd) {
                Some(entry) => real_lock(&entry.file, operation, &mut real.errno),
                None => {
                    real.errno = EBADF;
                    -1
                }
            }
        })
    }

    pub(super) fn real_lock_file_ex(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::LockFileEx {
            handle,
            flags,
            reserved: _,
            length_low: _,
            length_high: _,
            overlapped: _,
        } = call
        else {
            unreachable!("dispatched real_lock_file_ex")
        };
        Ok({
            // Win32 LockFileEx semantics over the provider's synthetic
            // handle. The exact byte range is intentionally ignored here:
            // the std wrapper always supplies offset zero + u64::MAX.
            let Some(fd) = synthetic_handle_fd(handle) else {
                self.real_fs_mut().errno = 6; // ERROR_INVALID_HANDLE
                return Ok(0);
            };
            if !self.require_real_descriptor_write_grant(fd, true) {
                return Ok(0);
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
        })
    }

    pub(super) fn real_unlock_file(&mut self, call: PreparedFilesystemCall) -> EvalResult<i64> {
        let PreparedFilesystemCall::UnlockFile {
            handle,
            offset_low: _,
            offset_high: _,
            length_low: _,
            length_high: _,
        } = call
        else {
            unreachable!("dispatched real_unlock_file")
        };
        Ok({
            let Some(fd) = synthetic_handle_fd(handle) else {
                self.real_fs_mut().errno = 6; // ERROR_INVALID_HANDLE
                return Ok(0);
            };
            if !self.require_real_descriptor_write_grant(fd, true) {
                return Ok(0);
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
        })
    }

    pub(super) fn real_change_file_owner(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<i64> {
        let PreparedFilesystemCall::ChangeFileOwner { fd, uid, gid } = call else {
            unreachable!("dispatched real_change_file_owner")
        };
        Ok({
            // `fchown(fd, uid, gid)`: by descriptor.
            if !self.require_real_descriptor_write_grant(fd, false) {
                return Ok(-1);
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
                        real.errno = super::ENOTSUP;
                        -1
                    }
                }
                None => {
                    real.errno = EBADF;
                    -1
                }
            }
        })
    }
}
