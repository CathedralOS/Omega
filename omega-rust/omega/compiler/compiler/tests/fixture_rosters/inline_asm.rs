//! Actual inline-assembly fixture inputs; execution and target assertions stay in the owner.

pub const ASM_FENCES_COMPILE: &str = "inline_asm/asm_fences_compile";
pub const ASM_INTERRUPT_CONTROL_COMPILE: &str = "inline_asm/asm_interrupt_control_compile";
pub const ASM_FLAGS_COMPILE: &str = "inline_asm/asm_flags_compile";
pub const ASM_MSR_COMPILE: &str = "inline_asm/asm_msr_compile";
pub const ASM_CONTROL_REGISTERS_COMPILE: &str = "inline_asm/asm_control_registers_compile";
pub const ASM_CLI_REQUIRES_MACHINE_AUTHORITY: &str =
    "inline_asm/asm_cli_requires_machine_authority";

pub const PASS_CANARIES: &[&str] = &[
    ASM_FENCES_COMPILE,
    ASM_INTERRUPT_CONTROL_COMPILE,
    ASM_FLAGS_COMPILE,
    ASM_MSR_COMPILE,
    ASM_CONTROL_REGISTERS_COMPILE,
];

pub const FAIL_CANARIES: &[&str] = &[ASM_CLI_REQUIRES_MACHINE_AUTHORITY];

pub const FLAGS_FAIL_CANARIES: &[(&str, &str)] = &[
    (
        "inline_asm/asm_popfq_requires_machine_authority",
        "asm instruction `popfq`, which requires a FREESTANDING boundary root",
    ),
    (
        "inline_asm/asm_pushfq_requires_u64_destination",
        "asm instruction `pushfq` operand `destination` requires an exact `u64` writable place",
    ),
    (
        "inline_asm/asm_popfq_requires_saved_place",
        "asm instruction `popfq` operand `saved flags` requires target register `rflags` constraint `u64`",
    ),
];

pub const MSR_FAIL_CANARIES: &[(&str, &str)] = &[
    (
        "inline_asm/asm_wrmsr_requires_machine_authority",
        "asm instruction `wrmsr`, which requires a FREESTANDING boundary root",
    ),
    (
        "inline_asm/asm_rdmsr_requires_u64_destination",
        "asm instruction `rdmsr` operand `destination` requires an exact `u64` writable place",
    ),
    (
        "inline_asm/asm_wrmsr_requires_u64_value",
        "asm instruction `wrmsr` operand `value` requires an exact `u64` for target register `edx:eax`, found `u32`",
    ),
];

pub const CONTROL_REGISTER_FAIL_CANARIES: &[(&str, &str)] = &[
    (
        "inline_asm/asm_write_cr3_requires_machine_authority",
        "asm instruction `write_cr3`, which requires a FREESTANDING boundary root",
    ),
    (
        "inline_asm/asm_read_cr3_requires_u64_destination",
        "asm instruction `read_cr3` operand `destination` requires an exact `u64` writable place",
    ),
    (
        "inline_asm/asm_write_cr3_requires_u64_value",
        "asm instruction `write_cr3` operand `value` requires an exact `u64` for target register `cr3`, found `u32`",
    ),
];
