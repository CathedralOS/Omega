/*
Host filesystem custody, platform metadata FFI, and exact-byte file publication.

We isolate the narrow OS interfaces that Rust's standard library does not
expose on stable across macOS, Linux, and Windows, returning closed boolean or
`(volume, object)` identity facts rather than raw OS handles or principal
names:
- `extended_acl_has_allow_entry` / `open_file_extended_acl_has_allow_entry`:
  inspects macOS extended ACLs (`<sys/acl.h>`) to reject `ACL_EXTENDED_ALLOW`
  entries that could broaden filesystem access beyond POSIX mode bits, while
  permitting deny-only (`ACL_EXTENDED_DENY`) entries. Darwin returns `ENOENT`
  both when a path is absent and when an existing object has no extended ACL,
  and returns `EINVAL` from `acl_get_entry` after the final entry; our wrapper
  disambiguates both cases so callers see `Ok(false)` for an existing object
  with no allow entries.
- `filesystem_object_identity`: returns `(dev, ino)` on Unix and
  `(volume_serial_number, file_index)` via `GetFileInformationByHandle` on
  Windows (opened with `access_mode(0)` and `FILE_FLAG_BACKUP_SEMANTICS` so
  directories and read-restricted files still report identity without requesting
  content access).
*/

/*
Submodules:
- `file_publication`: stages, installs, and byte-replays replaceable output
  files (`StagedFile`, `publish_exact_file_bytes`, `install_staged_files`).
- `lock_file`: opens persistent capability-relative lock files using exclusive
  `create_new` followed by non-creating reopen to avoid the Darwin 24.6 APFS
  `openat(..., O_CREAT)` race that returns spurious `ENOENT` under contention.
- `record_file`: capability-relative, symlink-safe record reads, exclusive
  stage-and-hard-link creation, atomic replacement, and advisory locking.
- `temporary_directory`: PID-and-sequence-scoped scratch directories under the
  host temporary root.
*/

#![deny(unsafe_op_in_unsafe_fn)]

pub mod file_publication;
pub mod lock_file;
pub mod record_file;
pub mod temporary_directory;

use std::{fs::File, io, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolicLinkBehavior {
    Follow,
    InspectLink,
}

pub fn extended_acl_has_allow_entry(
    path: &Path,
    symbolic_link_behavior: SymbolicLinkBehavior,
) -> io::Result<bool> {
    platform::extended_acl_has_allow_entry(path, symbolic_link_behavior)
}

pub fn open_file_extended_acl_has_allow_entry(file: &File) -> io::Result<bool> {
    platform::open_file_extended_acl_has_allow_entry(file)
}

pub fn filesystem_object_identity(path: &Path) -> io::Result<(u64, u64)> {
    link_custody::filesystem_object_identity(path)
}

#[cfg(target_os = "macos")]
mod platform {
    use super::SymbolicLinkBehavior;
    use std::ffi::{CString, c_char, c_int, c_void};
    use std::fs::File;
    use std::io;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    type Acl = *mut c_void;
    type AclEntry = *mut c_void;

    const ACL_TYPE_EXTENDED: c_int = 0x0000_0100;
    const ACL_FIRST_ENTRY: c_int = 0;
    const ACL_NEXT_ENTRY: c_int = -1;
    const ACL_EXTENDED_ALLOW: c_int = 1;
    const ACL_EXTENDED_DENY: c_int = 2;

    unsafe extern "C" {
        fn acl_free(value: *mut c_void) -> c_int;
        fn acl_get_entry(acl: Acl, entry_id: c_int, entry: *mut AclEntry) -> c_int;
        fn acl_get_fd_np(fd: c_int, kind: c_int) -> Acl;
        fn acl_get_file(path: *const c_char, kind: c_int) -> Acl;
        fn acl_get_link_np(path: *const c_char, kind: c_int) -> Acl;
        fn acl_get_tag_type(entry: AclEntry, tag: *mut c_int) -> c_int;
    }

    struct OwnedAcl(Acl);

    impl Drop for OwnedAcl {
        fn drop(&mut self) {
            // SAFETY: `self.0` is the non-null allocation returned by one ACL
            // getter and this guard performs its sole release.
            unsafe {
                let _ = acl_free(self.0);
            }
        }
    }

    pub(super) fn extended_acl_has_allow_entry(
        path: &Path,
        symbolic_link_behavior: SymbolicLinkBehavior,
    ) -> io::Result<bool> {
        let path_bytes = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "filesystem custody path contains an interior NUL byte",
            )
        })?;
        // SAFETY: `path_bytes` is a live NUL-terminated path. A non-null
        // returned allocation is immediately placed under one `acl_free` guard.
        let acl = unsafe {
            match symbolic_link_behavior {
                SymbolicLinkBehavior::Follow => {
                    acl_get_file(path_bytes.as_ptr(), ACL_TYPE_EXTENDED)
                }
                SymbolicLinkBehavior::InspectLink => {
                    acl_get_link_np(path_bytes.as_ptr(), ACL_TYPE_EXTENDED)
                }
            }
        };
        if acl.is_null() {
            let error = io::Error::last_os_error();
            let path_exists = match symbolic_link_behavior {
                SymbolicLinkBehavior::Follow => path.metadata().is_ok(),
                SymbolicLinkBehavior::InspectLink => path.symlink_metadata().is_ok(),
            };
            if error.kind() == io::ErrorKind::NotFound && path_exists {
                return Ok(false);
            }
            return Err(error);
        }
        acl_has_allow_entry(OwnedAcl(acl))
    }

    pub(super) fn open_file_extended_acl_has_allow_entry(file: &File) -> io::Result<bool> {
        // SAFETY: `file` keeps its live descriptor open for this call. A
        // non-null returned allocation is immediately placed under one
        // `acl_free` guard.
        let acl = unsafe { acl_get_fd_np(file.as_raw_fd(), ACL_TYPE_EXTENDED) };
        if acl.is_null() {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::NotFound {
                return Ok(false);
            }
            return Err(error);
        }
        acl_has_allow_entry(OwnedAcl(acl))
    }

    fn acl_has_allow_entry(acl: OwnedAcl) -> io::Result<bool> {
        let mut entry = std::ptr::null_mut();
        let mut selector = ACL_FIRST_ENTRY;
        let mut saw_entry = false;
        loop {
            // SAFETY: the owned ACL remains live and `entry` points to writable
            // storage for the borrowed entry handle.
            if unsafe { acl_get_entry(acl.0, selector, &mut entry) } != 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::InvalidInput && saw_entry {
                    return Ok(false);
                }
                return Err(error);
            }
            saw_entry = true;

            let mut tag = 0;
            // SAFETY: `entry` is borrowed from the live ACL and `tag` points to
            // writable storage of the ABI's declared integer type.
            if unsafe { acl_get_tag_type(entry, &mut tag) } != 0 {
                return Err(io::Error::last_os_error());
            }
            match tag {
                ACL_EXTENDED_ALLOW => return Ok(true),
                ACL_EXTENDED_DENY => {}
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "native extended ACL contains an unknown entry tag",
                    ));
                }
            }
            selector = ACL_NEXT_ENTRY;
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::SymbolicLinkBehavior;
    use std::{fs::File, io, path::Path};

    pub(super) fn extended_acl_has_allow_entry(
        _path: &Path,
        _symbolic_link_behavior: SymbolicLinkBehavior,
    ) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native extended ACL inspection is not implemented on this platform",
        ))
    }

    pub(super) fn open_file_extended_acl_has_allow_entry(_file: &File) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native extended ACL inspection is not implemented on this platform",
        ))
    }
}

#[cfg(unix)]
mod link_custody {
    use std::{io, os::unix::fs::MetadataExt, path::Path};

    pub(super) fn filesystem_object_identity(path: &Path) -> io::Result<(u64, u64)> {
        let metadata = path.metadata()?;
        Ok((metadata.dev(), metadata.ino()))
    }
}

#[cfg(windows)]
mod link_custody {
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::{AsRawHandle, RawHandle};
    use std::path::Path;

    #[repr(C)]
    struct ByHandleFileInformation {
        file_attributes: u32,
        creation_time: [u32; 2],
        last_access_time: [u32; 2],
        last_write_time: [u32; 2],
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

    unsafe extern "system" {
        fn GetFileInformationByHandle(
            file: RawHandle,
            information: *mut ByHandleFileInformation,
        ) -> i32;
    }

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_SHARE_READ_WRITE_DELETE: u32 = 0x0000_0007;

    fn open_for_metadata(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .access_mode(0)
            .share_mode(FILE_SHARE_READ_WRITE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
    }

    fn information(path: &Path) -> io::Result<ByHandleFileInformation> {
        let entry = open_for_metadata(path)?;
        let mut information = ByHandleFileInformation {
            file_attributes: 0,
            creation_time: [0; 2],
            last_access_time: [0; 2],
            last_write_time: [0; 2],
            volume_serial_number: 0,
            file_size_high: 0,
            file_size_low: 0,
            number_of_links: 0,
            file_index_high: 0,
            file_index_low: 0,
        };
        // SAFETY: `entry` owns the handle for the whole call, and
        // `information` is live writable storage of the declared record type.
        let read = unsafe { GetFileInformationByHandle(entry.as_raw_handle(), &mut information) };
        if read == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(information)
    }

    pub(super) fn filesystem_object_identity(path: &Path) -> io::Result<(u64, u64)> {
        let information = information(path)?;
        let index =
            (u64::from(information.file_index_high) << 32) | u64::from(information.file_index_low);
        Ok((u64::from(information.volume_serial_number), index))
    }
}

#[cfg(not(any(unix, windows)))]
mod link_custody {
    use std::{io, path::Path};

    pub(super) fn has_single_hard_link(_path: &Path) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "hard-link custody inspection is not implemented on this platform",
        ))
    }

    pub(super) fn filesystem_object_identity(_path: &Path) -> io::Result<(u64, u64)> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "filesystem object identity is not implemented on this platform",
        ))
    }
}
