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

/// Which Windows owner identities are acceptable for an inspected object.
///
/// Identity comparisons use binary SIDs obtained from the current process
/// token or created as well-known SIDs. No account-name lookup occurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsFileOwnerPolicy {
    /// Only the current process token's user may own the object.
    CurrentUserOnly,
    /// The current user, LocalSystem, or BUILTIN Administrators may own it.
    CurrentUserSystemOrAdministrators,
}

/// A closed reason that a Windows filesystem object failed custody inspection.
///
/// Variants deliberately do not expose native handles, SIDs, or principal
/// names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsFileCustodyViolation {
    UntrustedOwner,
    NullDacl,
    UntrustedMutationAuthority,
    UnsupportedAllowAce,
}

/// Inspect Windows owner and DACL custody through an already-open file handle.
///
/// The query is bound to `file`'s live native handle, so replacing a pathname
/// cannot redirect the observation. The DACL is accepted only when its owner
/// satisfies `owner_policy` and every access-allowing ACE that grants file or
/// directory mutation authority names the current user, LocalSystem, or
/// BUILTIN Administrators. Null DACLs and unsupported access-allowing ACE forms
/// are rejected. `Ok(None)` is the sole affirmative custody result. On
/// non-Windows targets this returns [`io::ErrorKind::Unsupported`].
pub fn inspect_open_windows_file_custody(
    file: &File,
    owner_policy: WindowsFileOwnerPolicy,
) -> io::Result<Option<WindowsFileCustodyViolation>> {
    windows_custody::inspect_open_file_custody(file, owner_policy)
}

#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowsPrincipalClass {
    CurrentUser,
    LocalSystem,
    Administrators,
    Other,
}

#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowsAllowingAce {
    NoMutation,
    TrustedMutation,
    UntrustedMutation,
    Unsupported,
}

#[cfg(any(windows, test))]
fn decide_windows_file_custody(
    owner_policy: WindowsFileOwnerPolicy,
    owner: WindowsPrincipalClass,
    dacl_present: bool,
    allowing_aces: &[WindowsAllowingAce],
) -> Option<WindowsFileCustodyViolation> {
    let owner_is_trusted = match owner_policy {
        WindowsFileOwnerPolicy::CurrentUserOnly => owner == WindowsPrincipalClass::CurrentUser,
        WindowsFileOwnerPolicy::CurrentUserSystemOrAdministrators => matches!(
            owner,
            WindowsPrincipalClass::CurrentUser
                | WindowsPrincipalClass::LocalSystem
                | WindowsPrincipalClass::Administrators
        ),
    };
    if !owner_is_trusted {
        return Some(WindowsFileCustodyViolation::UntrustedOwner);
    }
    if !dacl_present {
        return Some(WindowsFileCustodyViolation::NullDacl);
    }
    if allowing_aces.contains(&WindowsAllowingAce::Unsupported) {
        return Some(WindowsFileCustodyViolation::UnsupportedAllowAce);
    }
    if allowing_aces.contains(&WindowsAllowingAce::UntrustedMutation) {
        return Some(WindowsFileCustodyViolation::UntrustedMutationAuthority);
    }
    None
}

#[cfg(any(windows, test))]
const WINDOWS_GENERIC_ALL: u32 = 0x1000_0000;
#[cfg(any(windows, test))]
const WINDOWS_GENERIC_WRITE: u32 = 0x4000_0000;
#[cfg(any(windows, test))]
const WINDOWS_FILE_WRITE_DATA_OR_ADD_FILE: u32 = 0x0000_0002;
#[cfg(any(windows, test))]
const WINDOWS_FILE_APPEND_DATA_OR_ADD_SUBDIRECTORY: u32 = 0x0000_0004;
#[cfg(any(windows, test))]
const WINDOWS_FILE_WRITE_EA: u32 = 0x0000_0010;
#[cfg(any(windows, test))]
const WINDOWS_FILE_DELETE_CHILD: u32 = 0x0000_0040;
#[cfg(any(windows, test))]
const WINDOWS_FILE_WRITE_ATTRIBUTES: u32 = 0x0000_0100;
#[cfg(any(windows, test))]
const WINDOWS_DELETE: u32 = 0x0001_0000;
#[cfg(any(windows, test))]
const WINDOWS_WRITE_DAC: u32 = 0x0004_0000;
#[cfg(any(windows, test))]
const WINDOWS_WRITE_OWNER: u32 = 0x0008_0000;
#[cfg(any(windows, test))]
const WINDOWS_INHERIT_ONLY_ACE: u8 = 0x08;
#[cfg(any(windows, test))]
const WINDOWS_MUTATION_RIGHTS: u32 = WINDOWS_GENERIC_ALL
    | WINDOWS_GENERIC_WRITE
    | WINDOWS_FILE_WRITE_DATA_OR_ADD_FILE
    | WINDOWS_FILE_APPEND_DATA_OR_ADD_SUBDIRECTORY
    | WINDOWS_FILE_WRITE_EA
    | WINDOWS_FILE_DELETE_CHILD
    | WINDOWS_FILE_WRITE_ATTRIBUTES
    | WINDOWS_DELETE
    | WINDOWS_WRITE_DAC
    | WINDOWS_WRITE_OWNER;

#[cfg(any(windows, test))]
fn windows_access_mask_grants_mutation(access_mask: u32) -> bool {
    access_mask & WINDOWS_MUTATION_RIGHTS != 0
}

#[cfg(any(windows, test))]
fn windows_ace_applies_to_object(ace_flags: u8) -> bool {
    ace_flags & WINDOWS_INHERIT_ONLY_ACE == 0
}

#[cfg(windows)]
mod windows_custody {
    use super::{
        WindowsAllowingAce, WindowsFileCustodyViolation, WindowsFileOwnerPolicy,
        WindowsPrincipalClass, decide_windows_file_custody, windows_access_mask_grants_mutation,
        windows_ace_applies_to_object,
    };
    use std::ffi::c_void;
    use std::fs::File;
    use std::io;
    use std::mem::size_of;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, LocalFree};
    use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        ACE_INHERITED_OBJECT_TYPE_PRESENT, ACE_OBJECT_TYPE_PRESENT, ACL, ACL_SIZE_INFORMATION,
        AclSizeInformation, CreateWellKnownSid, DACL_SECURITY_INFORMATION, GetAce,
        GetAclInformation, GetLengthSid, GetTokenInformation, IsValidSid,
        OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SECURITY_MAX_SID_SIZE, TOKEN_QUERY,
        TOKEN_USER, TokenUser, WinBuiltinAdministratorsSid, WinLocalSystemSid,
    };
    use windows_sys::Win32::System::SystemServices::{
        ACCESS_ALLOWED_ACE_TYPE, ACCESS_ALLOWED_CALLBACK_ACE_TYPE,
        ACCESS_ALLOWED_CALLBACK_OBJECT_ACE_TYPE, ACCESS_ALLOWED_COMPOUND_ACE_TYPE,
        ACCESS_ALLOWED_OBJECT_ACE_TYPE, ACCESS_DENIED_ACE_TYPE, ACCESS_DENIED_CALLBACK_ACE_TYPE,
        ACCESS_DENIED_CALLBACK_OBJECT_ACE_TYPE, ACCESS_DENIED_OBJECT_ACE_TYPE,
        SYSTEM_ACCESS_FILTER_ACE_TYPE, SYSTEM_ALARM_ACE_TYPE, SYSTEM_ALARM_CALLBACK_ACE_TYPE,
        SYSTEM_ALARM_CALLBACK_OBJECT_ACE_TYPE, SYSTEM_ALARM_OBJECT_ACE_TYPE, SYSTEM_AUDIT_ACE_TYPE,
        SYSTEM_AUDIT_CALLBACK_ACE_TYPE, SYSTEM_AUDIT_CALLBACK_OBJECT_ACE_TYPE,
        SYSTEM_AUDIT_OBJECT_ACE_TYPE, SYSTEM_MANDATORY_LABEL_ACE_TYPE,
        SYSTEM_PROCESS_TRUST_LABEL_ACE_TYPE, SYSTEM_RESOURCE_ATTRIBUTE_ACE_TYPE,
        SYSTEM_SCOPED_POLICY_ID_ACE_TYPE,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    const ACE_HEADER_LENGTH: usize = 4;
    const BASIC_ALLOW_SID_OFFSET: usize = 8;
    const OBJECT_ALLOW_SID_BASE_OFFSET: usize = 12;
    const GUID_LENGTH: usize = 16;
    const SID_HEADER_LENGTH: usize = 8;

    struct LocalSecurityDescriptor(PSECURITY_DESCRIPTOR);

    impl Drop for LocalSecurityDescriptor {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: `GetSecurityInfo` returned this descriptor through
                // its ownership-transferring output parameter. This guard is
                // its sole owner and calls `LocalFree` exactly once.
                unsafe {
                    let _ = LocalFree(self.0);
                }
            }
        }
    }

    struct SidBytes {
        words: Vec<usize>,
        byte_len: usize,
    }

    impl SidBytes {
        fn new_capacity(byte_capacity: usize) -> Self {
            let word_count = byte_capacity.div_ceil(size_of::<usize>());
            Self {
                words: vec![0; word_count],
                byte_len: byte_capacity,
            }
        }

        fn as_psid(&self) -> PSID {
            self.words.as_ptr().cast_mut().cast()
        }

        fn as_bytes(&self) -> &[u8] {
            // SAFETY: `words` owns at least `byte_len` initialized bytes and
            // remains live for the returned borrow. Viewing initialized words
            // as bytes does not impose a stricter alignment requirement.
            unsafe { std::slice::from_raw_parts(self.words.as_ptr().cast(), self.byte_len) }
        }
    }

    struct TrustedSids {
        current_user: SidBytes,
        local_system: SidBytes,
        administrators: SidBytes,
    }

    impl TrustedSids {
        fn classify(&self, candidate: &[u8]) -> WindowsPrincipalClass {
            if candidate == self.current_user.as_bytes() {
                WindowsPrincipalClass::CurrentUser
            } else if candidate == self.local_system.as_bytes() {
                WindowsPrincipalClass::LocalSystem
            } else if candidate == self.administrators.as_bytes() {
                WindowsPrincipalClass::Administrators
            } else {
                WindowsPrincipalClass::Other
            }
        }
    }

    pub(super) fn inspect_open_file_custody(
        file: &File,
        owner_policy: WindowsFileOwnerPolicy,
    ) -> io::Result<Option<WindowsFileCustodyViolation>> {
        let trusted_sids = trusted_sids()?;
        let mut owner = null_mut();
        let mut dacl = null_mut();
        let mut raw_descriptor = null_mut();
        // SAFETY: `file` keeps the HANDLE live for this call. Every requested
        // output points to writable storage, unused outputs are null, and the
        // returned descriptor is immediately placed under a `LocalFree` guard.
        let query_result = unsafe {
            GetSecurityInfo(
                file.as_raw_handle().cast(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                null_mut(),
                &mut dacl,
                null_mut(),
                &mut raw_descriptor,
            )
        };
        let _descriptor = LocalSecurityDescriptor(raw_descriptor);
        if query_result != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(query_result as i32));
        }
        if raw_descriptor.is_null() {
            return Err(invalid_security_descriptor(
                "GetSecurityInfo returned no security descriptor",
            ));
        }

        let owner_class = if owner.is_null() {
            WindowsPrincipalClass::Other
        } else {
            trusted_sids.classify(copy_valid_sid(owner)?.as_bytes())
        };
        if dacl.is_null() {
            return Ok(decide_windows_file_custody(
                owner_policy,
                owner_class,
                false,
                &[],
            ));
        }

        let allowing_aces = inspect_dacl(dacl, &trusted_sids)?;
        Ok(decide_windows_file_custody(
            owner_policy,
            owner_class,
            true,
            &allowing_aces,
        ))
    }

    fn trusted_sids() -> io::Result<TrustedSids> {
        Ok(TrustedSids {
            current_user: current_process_user_sid()?,
            local_system: well_known_sid(WinLocalSystemSid)?,
            administrators: well_known_sid(WinBuiltinAdministratorsSid)?,
        })
    }

    fn current_process_user_sid() -> io::Result<SidBytes> {
        let mut raw_token = null_mut();
        // SAFETY: the process pseudo-handle is always valid in this process and
        // `raw_token` points to writable HANDLE storage.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `OpenProcessToken` succeeded and transferred one owned token
        // HANDLE. `OwnedHandle` performs its sole `CloseHandle` on drop.
        let token = unsafe { OwnedHandle::from_raw_handle(raw_token.cast()) };

        let mut byte_len = 0;
        // SAFETY: this is the documented sizing query: the token is live, the
        // output buffer is null with length zero, and `byte_len` is writable.
        let sizing_result = unsafe {
            GetTokenInformation(
                token.as_raw_handle().cast(),
                TokenUser,
                null_mut(),
                0,
                &mut byte_len,
            )
        };
        if sizing_result != 0 {
            return Err(invalid_security_descriptor(
                "current process token sizing query unexpectedly succeeded",
            ));
        }
        let sizing_error = io::Error::last_os_error();
        if sizing_error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
            return Err(sizing_error);
        }
        if byte_len < size_of::<TOKEN_USER>() as u32 {
            return Err(invalid_security_descriptor(
                "current process token returned an invalid user-SID size",
            ));
        }

        let mut token_information = SidBytes::new_capacity(byte_len as usize);
        let mut returned_len = byte_len;
        // SAFETY: the aligned allocation contains `byte_len` writable bytes,
        // the token remains live, and `returned_len` points to writable storage.
        if unsafe {
            GetTokenInformation(
                token.as_raw_handle().cast(),
                TokenUser,
                token_information.words.as_mut_ptr().cast(),
                byte_len,
                &mut returned_len,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if returned_len > byte_len || returned_len < size_of::<TOKEN_USER>() as u32 {
            return Err(invalid_security_descriptor(
                "current process token returned inconsistent user-SID data",
            ));
        }
        // SAFETY: the successful query initialized at least one aligned
        // `TOKEN_USER` in the retained allocation, as checked above.
        let token_user = unsafe { &*token_information.words.as_ptr().cast::<TOKEN_USER>() };
        copy_valid_sid(token_user.User.Sid)
    }

    fn well_known_sid(kind: i32) -> io::Result<SidBytes> {
        let mut sid = SidBytes::new_capacity(SECURITY_MAX_SID_SIZE as usize);
        let mut byte_len = SECURITY_MAX_SID_SIZE;
        // SAFETY: `sid` provides `byte_len` writable aligned bytes, no domain
        // SID is required for these absolute well-known SID types, and the
        // length output points to writable storage.
        if unsafe { CreateWellKnownSid(kind, null_mut(), sid.as_psid(), &mut byte_len) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if byte_len == 0 || byte_len > SECURITY_MAX_SID_SIZE {
            return Err(invalid_security_descriptor(
                "CreateWellKnownSid returned an invalid SID length",
            ));
        }
        sid.byte_len = byte_len as usize;
        Ok(sid)
    }

    fn copy_valid_sid(sid: PSID) -> io::Result<SidBytes> {
        if sid.is_null() {
            return Err(invalid_security_descriptor(
                "security descriptor contains a null SID",
            ));
        }
        // SAFETY: the SID pointer came from a successful Windows security API
        // query whose owning allocation remains live for this call.
        if unsafe { IsValidSid(sid) } == 0 {
            return Err(invalid_security_descriptor(
                "security descriptor contains an invalid SID",
            ));
        }
        // SAFETY: `IsValidSid` accepted this still-live SID allocation.
        let byte_len = unsafe { GetLengthSid(sid) } as usize;
        if !(SID_HEADER_LENGTH..=SECURITY_MAX_SID_SIZE as usize).contains(&byte_len) {
            return Err(invalid_security_descriptor(
                "security descriptor contains an invalid SID length",
            ));
        }
        let mut copied = SidBytes::new_capacity(byte_len);
        // SAFETY: both allocations are valid for `byte_len` bytes; the source
        // is the live validated SID and the fresh destination cannot overlap it.
        unsafe {
            std::ptr::copy_nonoverlapping(
                sid.cast::<u8>(),
                copied.words.as_mut_ptr().cast(),
                byte_len,
            );
        }
        copied.byte_len = byte_len;
        Ok(copied)
    }

    fn inspect_dacl(
        dacl: *mut ACL,
        trusted_sids: &TrustedSids,
    ) -> io::Result<Vec<WindowsAllowingAce>> {
        let mut information = ACL_SIZE_INFORMATION::default();
        // SAFETY: `dacl` belongs to the live security descriptor and
        // `information` is writable storage of the requested ABI type.
        if unsafe {
            GetAclInformation(
                dacl,
                (&mut information as *mut ACL_SIZE_INFORMATION).cast(),
                size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let acl_byte_len = information.AclBytesInUse as usize;
        if acl_byte_len < size_of::<ACL>() {
            return Err(invalid_security_descriptor(
                "DACL is shorter than its header",
            ));
        }

        let dacl_address = dacl as usize;
        let dacl_end = dacl_address
            .checked_add(acl_byte_len)
            .ok_or_else(|| invalid_security_descriptor("DACL address range overflowed"))?;
        let mut observations = Vec::with_capacity(information.AceCount as usize);
        for index in 0..information.AceCount {
            let mut raw_ace: *mut c_void = null_mut();
            // SAFETY: the DACL remains live and `raw_ace` points to writable
            // storage. `GetAclInformation` supplied the bounded ACE count.
            if unsafe { GetAce(dacl, index, &mut raw_ace) } == 0 {
                return Err(io::Error::last_os_error());
            }
            let ace_address = raw_ace as usize;
            if ace_address < dacl_address || ace_address >= dacl_end {
                return Err(invalid_security_descriptor(
                    "DACL returned an out-of-range ACE",
                ));
            }
            let remaining = dacl_end - ace_address;
            if remaining < ACE_HEADER_LENGTH {
                return Err(invalid_security_descriptor(
                    "DACL contains a truncated ACE header",
                ));
            }
            // SAFETY: the range checks above establish `remaining` readable
            // bytes inside the live DACL allocation.
            let remaining_bytes =
                unsafe { std::slice::from_raw_parts(raw_ace.cast::<u8>(), remaining) };
            let ace_size = u16::from_le_bytes([remaining_bytes[2], remaining_bytes[3]]) as usize;
            if ace_size < ACE_HEADER_LENGTH || ace_size > remaining {
                return Err(invalid_security_descriptor(
                    "DACL contains an invalid ACE size",
                ));
            }
            observations.push(inspect_ace(&remaining_bytes[..ace_size], trusted_sids));
        }
        Ok(observations)
    }

    fn inspect_ace(ace: &[u8], trusted_sids: &TrustedSids) -> WindowsAllowingAce {
        if !windows_ace_applies_to_object(ace[1]) {
            return WindowsAllowingAce::NoMutation;
        }
        let ace_type = u32::from(ace[0]);
        match ace_type {
            ACCESS_ALLOWED_ACE_TYPE | ACCESS_ALLOWED_CALLBACK_ACE_TYPE => {
                inspect_known_allowing_ace(ace, BASIC_ALLOW_SID_OFFSET, trusted_sids)
            }
            ACCESS_ALLOWED_OBJECT_ACE_TYPE | ACCESS_ALLOWED_CALLBACK_OBJECT_ACE_TYPE => {
                let Some(flags) = read_u32(ace, 8) else {
                    return WindowsAllowingAce::Unsupported;
                };
                if flags & !(ACE_OBJECT_TYPE_PRESENT | ACE_INHERITED_OBJECT_TYPE_PRESENT) != 0 {
                    return WindowsAllowingAce::Unsupported;
                }
                let sid_offset = OBJECT_ALLOW_SID_BASE_OFFSET
                    + usize::from(flags & ACE_OBJECT_TYPE_PRESENT != 0) * GUID_LENGTH
                    + usize::from(flags & ACE_INHERITED_OBJECT_TYPE_PRESENT != 0) * GUID_LENGTH;
                inspect_known_allowing_ace(ace, sid_offset, trusted_sids)
            }
            ACCESS_ALLOWED_COMPOUND_ACE_TYPE => WindowsAllowingAce::Unsupported,
            ACCESS_DENIED_ACE_TYPE
            | ACCESS_DENIED_OBJECT_ACE_TYPE
            | ACCESS_DENIED_CALLBACK_ACE_TYPE
            | ACCESS_DENIED_CALLBACK_OBJECT_ACE_TYPE
            | SYSTEM_AUDIT_ACE_TYPE
            | SYSTEM_AUDIT_OBJECT_ACE_TYPE
            | SYSTEM_AUDIT_CALLBACK_ACE_TYPE
            | SYSTEM_AUDIT_CALLBACK_OBJECT_ACE_TYPE
            | SYSTEM_ALARM_ACE_TYPE
            | SYSTEM_ALARM_OBJECT_ACE_TYPE
            | SYSTEM_ALARM_CALLBACK_ACE_TYPE
            | SYSTEM_ALARM_CALLBACK_OBJECT_ACE_TYPE
            | SYSTEM_MANDATORY_LABEL_ACE_TYPE
            | SYSTEM_RESOURCE_ATTRIBUTE_ACE_TYPE
            | SYSTEM_SCOPED_POLICY_ID_ACE_TYPE
            | SYSTEM_PROCESS_TRUST_LABEL_ACE_TYPE
            | SYSTEM_ACCESS_FILTER_ACE_TYPE => WindowsAllowingAce::NoMutation,
            _ => WindowsAllowingAce::Unsupported,
        }
    }

    fn inspect_known_allowing_ace(
        ace: &[u8],
        sid_offset: usize,
        trusted_sids: &TrustedSids,
    ) -> WindowsAllowingAce {
        let Some(access_mask) = read_u32(ace, 4) else {
            return WindowsAllowingAce::Unsupported;
        };
        let Some(sid_tail) = ace.get(sid_offset..) else {
            return WindowsAllowingAce::Unsupported;
        };
        let Some(sid_len) = sid_byte_len(sid_tail) else {
            return WindowsAllowingAce::Unsupported;
        };
        if !windows_access_mask_grants_mutation(access_mask) {
            return WindowsAllowingAce::NoMutation;
        }
        if matches!(
            trusted_sids.classify(&sid_tail[..sid_len]),
            WindowsPrincipalClass::CurrentUser
                | WindowsPrincipalClass::LocalSystem
                | WindowsPrincipalClass::Administrators
        ) {
            WindowsAllowingAce::TrustedMutation
        } else {
            WindowsAllowingAce::UntrustedMutation
        }
    }

    fn sid_byte_len(bytes: &[u8]) -> Option<usize> {
        let subauthority_count = usize::from(*bytes.get(1)?);
        if subauthority_count > 15 {
            return None;
        }
        let byte_len = SID_HEADER_LENGTH.checked_add(subauthority_count.checked_mul(4)?)?;
        (bytes.len() >= byte_len).then_some(byte_len)
    }

    fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
        let bytes: [u8; 4] = bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
        Some(u32::from_le_bytes(bytes))
    }

    fn invalid_security_descriptor(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, message)
    }
}

#[cfg(not(windows))]
mod windows_custody {
    use super::{WindowsFileCustodyViolation, WindowsFileOwnerPolicy};
    use std::fs::File;
    use std::io;

    pub(super) fn inspect_open_file_custody(
        _file: &File,
        _owner_policy: WindowsFileOwnerPolicy,
    ) -> io::Result<Option<WindowsFileCustodyViolation>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows security-descriptor inspection is unavailable on this platform",
        ))
    }
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

#[cfg(test)]
mod windows_custody_decision_tests {
    use super::{
        WINDOWS_DELETE, WINDOWS_FILE_APPEND_DATA_OR_ADD_SUBDIRECTORY, WINDOWS_FILE_DELETE_CHILD,
        WINDOWS_FILE_WRITE_ATTRIBUTES, WINDOWS_FILE_WRITE_DATA_OR_ADD_FILE, WINDOWS_FILE_WRITE_EA,
        WINDOWS_GENERIC_ALL, WINDOWS_GENERIC_WRITE, WINDOWS_WRITE_DAC, WINDOWS_WRITE_OWNER,
        WindowsAllowingAce, WindowsFileCustodyViolation, WindowsFileOwnerPolicy,
        WindowsPrincipalClass, decide_windows_file_custody, windows_access_mask_grants_mutation,
        windows_ace_applies_to_object,
    };

    #[test]
    fn cache_owner_policy_accepts_only_the_current_user() {
        assert_eq!(
            decide_windows_file_custody(
                WindowsFileOwnerPolicy::CurrentUserOnly,
                WindowsPrincipalClass::CurrentUser,
                true,
                &[],
            ),
            None
        );
        for owner in [
            WindowsPrincipalClass::LocalSystem,
            WindowsPrincipalClass::Administrators,
            WindowsPrincipalClass::Other,
        ] {
            assert_eq!(
                decide_windows_file_custody(
                    WindowsFileOwnerPolicy::CurrentUserOnly,
                    owner,
                    true,
                    &[],
                ),
                Some(WindowsFileCustodyViolation::UntrustedOwner)
            );
        }
    }

    #[test]
    fn executable_owner_policy_accepts_the_three_named_trust_anchors() {
        for owner in [
            WindowsPrincipalClass::CurrentUser,
            WindowsPrincipalClass::LocalSystem,
            WindowsPrincipalClass::Administrators,
        ] {
            assert_eq!(
                decide_windows_file_custody(
                    WindowsFileOwnerPolicy::CurrentUserSystemOrAdministrators,
                    owner,
                    true,
                    &[],
                ),
                None
            );
        }
        assert_eq!(
            decide_windows_file_custody(
                WindowsFileOwnerPolicy::CurrentUserSystemOrAdministrators,
                WindowsPrincipalClass::Other,
                true,
                &[],
            ),
            Some(WindowsFileCustodyViolation::UntrustedOwner)
        );
    }

    #[test]
    fn null_dacl_and_allowing_ace_failures_remain_distinct() {
        assert_eq!(
            decide_windows_file_custody(
                WindowsFileOwnerPolicy::CurrentUserOnly,
                WindowsPrincipalClass::CurrentUser,
                false,
                &[],
            ),
            Some(WindowsFileCustodyViolation::NullDacl)
        );
        assert_eq!(
            decide_windows_file_custody(
                WindowsFileOwnerPolicy::CurrentUserOnly,
                WindowsPrincipalClass::CurrentUser,
                true,
                &[WindowsAllowingAce::UntrustedMutation],
            ),
            Some(WindowsFileCustodyViolation::UntrustedMutationAuthority)
        );
        assert_eq!(
            decide_windows_file_custody(
                WindowsFileOwnerPolicy::CurrentUserOnly,
                WindowsPrincipalClass::CurrentUser,
                true,
                &[WindowsAllowingAce::Unsupported],
            ),
            Some(WindowsFileCustodyViolation::UnsupportedAllowAce)
        );
    }

    #[test]
    fn trusted_mutation_and_untrusted_read_only_aces_do_not_reject_custody() {
        assert_eq!(
            decide_windows_file_custody(
                WindowsFileOwnerPolicy::CurrentUserOnly,
                WindowsPrincipalClass::CurrentUser,
                true,
                &[
                    WindowsAllowingAce::TrustedMutation,
                    WindowsAllowingAce::NoMutation,
                ],
            ),
            None
        );
    }

    #[test]
    fn mutation_mask_covers_file_directory_and_security_descriptor_writes() {
        for right in [
            WINDOWS_GENERIC_ALL,
            WINDOWS_GENERIC_WRITE,
            WINDOWS_FILE_WRITE_DATA_OR_ADD_FILE,
            WINDOWS_FILE_APPEND_DATA_OR_ADD_SUBDIRECTORY,
            WINDOWS_FILE_WRITE_EA,
            WINDOWS_FILE_DELETE_CHILD,
            WINDOWS_FILE_WRITE_ATTRIBUTES,
            WINDOWS_DELETE,
            WINDOWS_WRITE_DAC,
            WINDOWS_WRITE_OWNER,
        ] {
            assert!(windows_access_mask_grants_mutation(right));
        }
        assert!(!windows_access_mask_grants_mutation(0x0012_0089));
    }

    #[test]
    fn inherit_only_aces_do_not_apply_to_the_current_object() {
        assert!(windows_ace_applies_to_object(0));
        assert!(!windows_ace_applies_to_object(0x08));
        assert!(!windows_ace_applies_to_object(0x0b));
    }
}

#[cfg(all(test, windows))]
mod windows_native_tests {
    use super::{
        WindowsFileCustodyViolation, WindowsFileOwnerPolicy, inspect_open_windows_file_custody,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn inspects_a_new_file_through_its_retained_handle() {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "omega-platform-custody-windows-{}-{sequence}",
            std::process::id()
        ));
        let file = std::fs::File::create(&path).expect("create Windows custody test file");
        let violation =
            inspect_open_windows_file_custody(&file, WindowsFileOwnerPolicy::CurrentUserOnly)
                .expect("inspect Windows security descriptor");
        assert_ne!(violation, Some(WindowsFileCustodyViolation::UntrustedOwner));
        assert_ne!(violation, Some(WindowsFileCustodyViolation::NullDacl));
        drop(file);
        std::fs::remove_file(path).expect("remove Windows custody test file");
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
