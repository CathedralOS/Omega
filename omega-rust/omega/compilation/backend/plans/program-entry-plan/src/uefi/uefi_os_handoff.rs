//! UEFI OS-handoff exhaustion status semantics.

const EFI_STATUS_ERROR_BIT: u64 = 1_u64 << 63;

/// Whether a deployment-supplied exhaustion status is a target-authored UEFI
/// error. The bounded retry loop must never report success or a warning after
/// every attempt was spent on a stale key, so the status the invocation's
/// firmware return reports on exhaustion has to carry the UEFI error bit.
pub const fn uefi_os_handoff_exhaustion_requires_error(exhaustion_status: u64) -> bool {
    exhaustion_status & EFI_STATUS_ERROR_BIT != 0
}

