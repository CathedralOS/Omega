//! Actual inline-assembly fixture inputs; execution and target assertions stay in the owner.

pub const ASM_FENCES_COMPILE: &str = "inline_asm/asm_fences_compile";
pub const ASM_INTERRUPT_CONTROL_COMPILE: &str = "inline_asm/asm_interrupt_control_compile";
pub const ASM_FLAGS_COMPILE: &str = "inline_asm/asm_flags_compile";
pub const ASM_MSR_COMPILE: &str = "inline_asm/asm_msr_compile";
pub const ASM_CONTROL_REGISTERS_COMPILE: &str = "inline_asm/asm_control_registers_compile";
pub const ASM_REGISTER_MOVE_COMPILE: &str = "inline_asm/asm_register_move_compile";
pub const ASM_MEMORY_TRANSFER_COMPILE: &str = "inline_asm/asm_memory_transfer_compile";
pub const ASM_X86_PIPELINE_DIRECTIVES_COMPILE: &str =
    "inline_asm/asm_x86_pipeline_directives_compile";
pub const ASM_AARCH64_PIPELINE_DIRECTIVES_COMPILE: &str =
    "inline_asm/asm_aarch64_pipeline_directives_compile";
pub const ASM_CACHE_MAINTENANCE_COMPILE: &str = "inline_asm/asm_cache_maintenance_compile";
pub const ASM_CLI_REQUIRES_MACHINE_AUTHORITY: &str =
    "inline_asm/asm_cli_requires_machine_authority";
pub const ASM_WBINVD_REQUIRES_MACHINE_AUTHORITY: &str =
    "inline_asm/asm_wbinvd_requires_machine_authority";
pub const ASM_INVD_REQUIRES_MACHINE_AUTHORITY: &str =
    "inline_asm/asm_invd_requires_machine_authority";
pub const ASM_WBNOINVD_REQUIRES_MACHINE_AUTHORITY: &str =
    "inline_asm/asm_wbnoinvd_requires_machine_authority";

pub const PASS_CANARIES: &[&str] = &[
    ASM_FENCES_COMPILE,
    ASM_INTERRUPT_CONTROL_COMPILE,
    ASM_FLAGS_COMPILE,
    ASM_MSR_COMPILE,
    ASM_CONTROL_REGISTERS_COMPILE,
    ASM_REGISTER_MOVE_COMPILE,
    ASM_MEMORY_TRANSFER_COMPILE,
    ASM_X86_PIPELINE_DIRECTIVES_COMPILE,
    ASM_AARCH64_PIPELINE_DIRECTIVES_COMPILE,
    ASM_CACHE_MAINTENANCE_COMPILE,
];

pub const FAIL_CANARIES: &[&str] = &[
    ASM_CLI_REQUIRES_MACHINE_AUTHORITY,
    ASM_PAUSE_REJECTS_OPERANDS,
    ASM_SERIALIZE_REJECTS_CLOBBER_CONTRACT,
    ASM_WBINVD_REQUIRES_MACHINE_AUTHORITY,
    ASM_INVD_REQUIRES_MACHINE_AUTHORITY,
    ASM_WBNOINVD_REQUIRES_MACHINE_AUTHORITY,
];

pub const ASM_PAUSE_REJECTS_OPERANDS: &str = "inline_asm/asm_pause_rejects_operands";
pub const ASM_SERIALIZE_REJECTS_CLOBBER_CONTRACT: &str =
    "inline_asm/asm_serialize_rejects_clobber_contract";

/// Pipeline-directive controls pin the zero-operand statement form and the
/// empty clobber contract; fragments live inline because these fixtures are
/// exercised through `assert_contract_rejects`.
pub const PIPELINE_DIRECTIVE_FAIL_CANARIES: &[(&str, &str)] = &[
    (
        ASM_PAUSE_REJECTS_OPERANDS,
        "multiple asm instructions must be separated by `;`",
    ),
    (
        ASM_SERIALIZE_REJECTS_CLOBBER_CONTRACT,
        "not clobbered `rax`",
    ),
];

/// The cache-maintenance operations are privileged: hosted programs naming
/// MachineControl still refuse because they do not own the machine.
pub const CACHE_OPERATION_FAIL_CANARIES: &[(&str, &str)] = &[
    (
        ASM_WBINVD_REQUIRES_MACHINE_AUTHORITY,
        "asm instruction `wbinvd`, which requires a FREESTANDING boundary root",
    ),
    (
        ASM_INVD_REQUIRES_MACHINE_AUTHORITY,
        "asm instruction `invd`, which requires a FREESTANDING boundary root",
    ),
    (
        ASM_WBNOINVD_REQUIRES_MACHINE_AUTHORITY,
        "asm instruction `wbnoinvd`, which requires a FREESTANDING boundary root",
    ),
];

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
