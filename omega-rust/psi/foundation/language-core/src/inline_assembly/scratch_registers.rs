//! Closed modeled scratch views shared by source decoding and CFG versions.
//! A 32-bit write also changes its 64-bit containing register. Other aliases
//! are not independent variables: they stay refused until shared-bit transfer
//! is modeled. SP/BP views are excluded from this ordinary value-only cohort.
pub const ASM_SCRATCH_SLOT_COUNT: usize = 51;
pub const ASM_ZERO_FLAG_SLOT: usize = 14;
pub const ASM_SCRATCH_NAMES: [&str; ASM_SCRATCH_SLOT_COUNT] = [
    "eax",
    "ebx",
    "ecx",
    "edx",
    "esi",
    "edi",
    "r8d",
    "r9d",
    "r10d",
    "r11d",
    "r12d",
    "r13d",
    "r14d",
    "r15d",
    "zf",
    "asm#flags",
    "asm#flags_environment",
    "asm#sub_left",
    "asm#sub_right",
    "asm#flags_mask",
    "asm#flags_snapshot",
    "asm#comparison_result",
    "w0",
    "w1",
    "w2",
    "w3",
    "w4",
    "w5",
    "w6",
    "w7",
    "w8",
    "w9",
    "w10",
    "w11",
    "w12",
    "w13",
    "w14",
    "w15",
    "w16",
    "w17",
    "w18",
    "w19",
    "w20",
    "w21",
    "w22",
    "w23",
    "w24",
    "w25",
    "w26",
    "w27",
    "w28",
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

/// AArch64 Wn writes zero-extend Xn. Xn and narrow/stack aliases remain
/// unavailable until shared-bit transfer is modeled, as for x86 aliases.
pub const ASM_ARM_REGISTER_START: usize = 22;
pub const ASM_ARM_CLOBBERS: [&str; 29] = [
    "x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8", "x9", "x10", "x11", "x12", "x13", "x14",
    "x15", "x16", "x17", "x18", "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26", "x27",
    "x28",
];

pub fn scratch_register(
    name: &str,
) -> Option<(usize, super::AsmTargetApplicability, &'static str)> {
    ASM_SCRATCH_NAMES
        .iter()
        .position(|candidate| *candidate == name)
        .and_then(|index| {
            if index < ASM_ZERO_FLAG_SLOT {
                Some((
                    index,
                    super::AsmTargetApplicability::X86_64,
                    ASM_SCRATCH_CLOBBERS[index],
                ))
            } else if index >= ASM_ARM_REGISTER_START {
                Some((
                    index,
                    super::AsmTargetApplicability::Aarch64,
                    ASM_ARM_CLOBBERS[index - ASM_ARM_REGISTER_START],
                ))
            } else {
                None
            }
        })
}

/// The 64-bit aliases and architectural SP/zero views have different read and
/// write rules; they are not independent u32 scratch cells.
pub fn is_unmodeled_aarch64_scratch_view(name: &str) -> bool {
    matches!(name, "wsp" | "xzr" | "wzr" | "x31" | "w31" | "w29" | "w30")
        || name
            .strip_prefix('x')
            .is_some_and(|number| number.parse::<u8>().is_ok_and(|register| register <= 30))
}
