//! Closed modeled scratch views shared by source decoding and CFG versions.
//! A 32-bit write also changes its 64-bit containing register. Other aliases
//! are not independent variables: they stay refused until shared-bit transfer
//! is modeled. SP/BP views are excluded from this ordinary value-only cohort.
pub const ASM_SCRATCH_SLOT_COUNT: usize = 15;
pub const ASM_ZERO_FLAG_SLOT: usize = 14;
pub const ASM_SCRATCH_NAMES: [&str; ASM_SCRATCH_SLOT_COUNT] = [
    "eax", "ebx", "ecx", "edx", "esi", "edi", "r8d", "r9d", "r10d", "r11d", "r12d", "r13d", "r14d",
    "r15d", "zf",
];
pub const ASM_SCRATCH_CLOBBERS: [&str; ASM_ZERO_FLAG_SLOT] = [
    "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15",
];

pub fn is_unmodeled_x86_scratch_view(name: &str) -> bool {
    matches!(
        name,
        "rax"
            | "rbx"
            | "rcx"
            | "rdx"
            | "rsi"
            | "rdi"
            | "rsp"
            | "rbp"
            | "esp"
            | "ebp"
            | "ax"
            | "bx"
            | "cx"
            | "dx"
            | "si"
            | "di"
            | "sp"
            | "bp"
            | "al"
            | "ah"
            | "bl"
            | "bh"
            | "cl"
            | "ch"
            | "dl"
            | "dh"
            | "sil"
            | "dil"
            | "spl"
            | "bpl"
            | "r8"
            | "r9"
            | "r10"
            | "r11"
            | "r12"
            | "r13"
            | "r14"
            | "r15"
            | "r8w"
            | "r9w"
            | "r10w"
            | "r11w"
            | "r12w"
            | "r13w"
            | "r14w"
            | "r15w"
            | "r8b"
            | "r9b"
            | "r10b"
            | "r11b"
            | "r12b"
            | "r13b"
            | "r14b"
            | "r15b"
            | "rflags"
            | "eflags"
            | "flags"
            | "cf"
            | "of"
            | "sf"
            | "af"
            | "pf"
    )
}
