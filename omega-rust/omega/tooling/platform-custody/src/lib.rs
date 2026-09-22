//! Narrow native filesystem-custody observations used by privileged tooling.
//!
//! This crate owns the platform FFI required to inspect metadata that Rust's
//! standard library does not expose. Callers receive closed facts, never native
//! handles or attacker-controlled principal names.

#![deny(unsafe_op_in_unsafe_fn)]

pub mod record_file;

use std::fs::File;
use std::io;
use std::path::Path;

/// Whether an ACL query follows a symbolic link or inspects the link itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolicLinkBehavior {
    Follow,
    InspectLink,
}

/// Report whether the native extended ACL contains any allow entry.
///
/// Deny-only ACLs cannot broaden filesystem authority and return `false`.
/// Unsupported platforms and filesystems return an error rather than a false
/// custody result.
pub fn extended_acl_has_allow_entry(
    path: &Path,
    symbolic_link_behavior: SymbolicLinkBehavior,
) -> io::Result<bool> {
    platform::extended_acl_has_allow_entry(path, symbolic_link_behavior)
}

/// Report whether an already-open filesystem object has any native extended
/// ACL allow entry.
///
/// The descriptor, rather than a display pathname, selects the inspected
/// object. Deny-only ACLs cannot broaden filesystem authority and return
/// `false`. Unsupported platforms and filesystems return an error.
pub fn open_file_extended_acl_has_allow_entry(file: &File) -> io::Result<bool> {
    platform::open_file_extended_acl_has_allow_entry(file)
}

/// Report whether a filesystem object is reachable through exactly one
/// directory entry.
///
/// An object with a second hard link shares its storage with a name the caller
/// does not own, so rewriting its attributes reaches custody outside the
/// caller's tree. Unix reads the count from the entry's own metadata; Windows
/// reads it from the opened object, which the standard library exposes only
/// behind an unstable interface. Symbolic links are the caller's separate
/// question: this reports on the object the path names.
pub fn has_single_hard_link(path: &Path) -> io::Result<bool> {
    link_custody::has_single_hard_link(path)
}

/// Report the filesystem object a path currently names, as an opaque
/// volume-and-object pair.
///
/// Two paths naming one object report equal pairs, so a caller can tell a
/// renamed-and-recreated directory from the original at the same spelling. The
/// pair is identity only: neither half is an address, a size, or ordered
/// against another volume's. Unix reads device and inode; Windows reads the
/// volume serial and file index, which the standard library exposes only
/// behind an unstable interface.
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

    // Stable Darwin constants from <sys/acl.h>. Keeping this vocabulary closed
    // avoids resolving ACL principals through ambient directory services.
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
            // Darwin reports ENOENT both when the path is absent and when an
            // existing object has no extended ACL. Recheck existence using
            // the same link-following policy to distinguish the empty ACL.
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
            // Darwin uses ENOENT for an existing descriptor whose object has
            // no extended ACL. Unlike the path query, the live descriptor
            // itself already establishes object existence.
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
                // Darwin reports EINVAL after the final entry rather than a
                // separate end-of-sequence return value.
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
    use std::fs::File;
    use std::io;
    use std::path::Path;

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
    use std::io;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    pub(super) fn has_single_hard_link(path: &Path) -> io::Result<bool> {
        Ok(path.symlink_metadata()?.nlink() == 1)
    }

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

    /// `BY_HANDLE_FILE_INFORMATION` from `<fileapi.h>`. Only the link count is
    /// read; the remaining members retain the record's exact declared size.
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

    // A directory handle needs backup semantics, and identity and link count
    // are metadata rather than content, so the open requests no access at all.
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

    pub(super) fn has_single_hard_link(path: &Path) -> io::Result<bool> {
        Ok(information(path)?.number_of_links == 1)
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
    use std::io;
    use std::path::Path;

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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::{
        SymbolicLinkBehavior, extended_acl_has_allow_entry, open_file_extended_acl_has_allow_entry,
    };
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn change_acl(path: &std::path::Path, arguments: &[&str]) {
        let status = Command::new("/bin/chmod")
            .args(arguments)
            .arg(path)
            .status()
            .expect("run the concrete macOS ACL editor");
        assert!(status.success(), "ACL edit failed for {}", path.display());
    }

    #[test]
    fn distinguishes_allow_entries_from_empty_and_deny_only_acls() {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "omega-platform-custody-acl-{}-{sequence}",
            std::process::id()
        ));
        std::fs::write(&path, b"custody").expect("create ACL test file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("make ACL test file private");

        assert!(
            !extended_acl_has_allow_entry(&path, SymbolicLinkBehavior::Follow)
                .expect("inspect empty ACL")
        );
        let file = std::fs::File::open(&path).expect("retain ACL test file");
        assert!(
            !open_file_extended_acl_has_allow_entry(&file).expect("inspect empty ACL by handle")
        );
        change_acl(&path, &["+a", "everyone allow write"]);
        assert!(
            extended_acl_has_allow_entry(&path, SymbolicLinkBehavior::Follow)
                .expect("inspect allow ACL")
        );
        assert!(
            open_file_extended_acl_has_allow_entry(&file).expect("inspect allow ACL by handle")
        );
        change_acl(&path, &["-N"]);
        change_acl(&path, &["+a", "everyone deny write"]);
        assert!(
            !extended_acl_has_allow_entry(&path, SymbolicLinkBehavior::Follow)
                .expect("inspect deny-only ACL")
        );
        assert!(
            !open_file_extended_acl_has_allow_entry(&file)
                .expect("inspect deny-only ACL by handle")
        );

        change_acl(&path, &["-N"]);
        std::fs::remove_file(&path).expect("remove ACL test file");
    }

    #[test]
    fn open_file_query_remains_bound_to_the_retained_object() {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "omega-platform-custody-retained-acl-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&root).expect("create retained ACL test root");
        let path = root.join("observed");
        let retained = root.join("retained");
        std::fs::write(&path, b"retained").expect("create retained ACL test file");
        let file = std::fs::File::open(&path).expect("retain ACL test file");

        std::fs::rename(&path, &retained).expect("relocate retained ACL test file");
        std::fs::write(&path, b"replacement").expect("create replacement ACL test file");
        change_acl(&path, &["+a", "everyone allow write"]);
        assert!(
            !open_file_extended_acl_has_allow_entry(&file)
                .expect("inspect retained object rather than replacement path")
        );

        change_acl(&retained, &["+a", "everyone allow write"]);
        assert!(
            open_file_extended_acl_has_allow_entry(&file)
                .expect("inspect allow ACL on retained object")
        );

        change_acl(&path, &["-N"]);
        change_acl(&retained, &["-N"]);
        std::fs::remove_dir_all(&root).expect("remove retained ACL test root");
    }
}
