//! Narrow native filesystem-custody observations used by privileged tooling.
//!
//! This crate owns the platform FFI required to inspect metadata that Rust's
//! standard library does not expose. Callers receive closed facts, never native
//! handles or attacker-controlled principal names.

#![deny(unsafe_op_in_unsafe_fn)]

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
