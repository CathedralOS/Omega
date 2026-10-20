use target::NativeTarget;

/// Function-level proof that ambient x86 floating controls were saved,
/// Omega's canonical nearest-even/gradual/masked MXCSR was installed, and the
/// complete incoming control state was restored before return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86FloatingControlRecord {
    pub target: NativeTarget,
    pub canonical_mxcsr: u32,
    pub canonical_slot_byte_offset: u32,
    pub saved_slot_byte_offset: u32,
    pub save_offset: usize,
    pub save_byte_count: usize,
    pub canonical_store_offset: usize,
    pub canonical_store_byte_count: usize,
    pub install_offset: usize,
    pub install_byte_count: usize,
    pub restore_offset: usize,
    pub restore_byte_count: usize,
}

/// Per-call proof that one returning x86 foreign boundary preserved the
/// caller's complete MXCSR. The slot may be reused by sequential calls, while
/// each call retains distinct save/restore instruction intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86ForeignCallFloatingControlRecord {
    pub target: NativeTarget,
    pub saved_slot_byte_offset: u32,
    pub save_offset: usize,
    pub save_byte_count: usize,
    pub restore_offset: usize,
    pub restore_byte_count: usize,
}
