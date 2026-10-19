//! Host processor-state observation for explicitly installed interpreter models.
//! This is a native mechanism leaf, not a source-instruction callback: the
//! compiler owns PUSHFQ meaning and the interpreter owns the supplied state.

/// Capture this host's x86 PUSHFQ image. Other architectures supply no image;
/// an embedding sponsor must select a separate modeled x86 environment there.
pub fn capture_host_x86_flags() -> Option<u64> {
    #[cfg(target_arch = "x86_64")]
    {
        let image: u64;
        // SAFETY: PUSHFQ/POP are unprivileged and balanced. The sequence uses
        // eight bytes of compiler-managed stack, so nostack/nomem are absent.
        // It preserves flags and every register except the declared output.
        unsafe {
            core::arch::asm!("pushfq", "pop {}", out(reg) image, options(preserves_flags));
        }
        Some(image)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        None
    }
}
