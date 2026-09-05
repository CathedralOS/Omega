//! Exact fixture identities and execution tables owned by entry/ABI tests.

pub const BUILD_EXPLICIT_PROGRAM_ENTRY_BINDING: &str = "build/explicit_program_entry_binding";
pub const BUILD_UEFI_PROGRAM_ENTRY_STORAGE_ROOTS: &str = "build/uefi_program_entry_storage_roots";
pub const ARITHMETIC_RUNTIME_CHAINED_FIELD_MUTATION_EXIT: &str =
    "arithmetic/runtime_chained_field_mutation_exit";
pub const INLINE_ASM_ASM_FENCES_COMPILE: &str = "inline_asm/asm_fences_compile";
pub const INLINE_ASM_ASM_PORT_OUT_FINAL_VALIDATION: &str =
    "inline_asm/asm_port_out_final_validation";
pub const TARGETS_AARCH64_HFA_ENTRY_ARGUMENT: &str = "targets/aarch64_hfa_entry_argument";
pub const TARGETS_AARCH64_SMALL_AGGREGATE_ENTRY: &str = "targets/aarch64_small_aggregate_entry";
pub const TARGETS_AARCH64_SMALL_AGGREGATE_STACK_ENTRY: &str =
    "targets/aarch64_small_aggregate_stack_entry";
pub const TARGETS_AARCH64_LARGE_AGGREGATE_ENTRY: &str = "targets/aarch64_large_aggregate_entry";
pub const TARGETS_AARCH64_LARGE_AGGREGATE_STACK_ENTRY: &str =
    "targets/aarch64_large_aggregate_stack_entry";
pub const TARGETS_AARCH64_WIDE_AGGREGATE_ENTRY: &str = "targets/aarch64_wide_aggregate_entry";
pub const TARGETS_AARCH64_SMALL_RESULT_ENTRY: &str = "targets/aarch64_small_result_entry";
pub const TARGETS_AGGREGATE_LITERAL_RESULT_ENTRY: &str = "targets/aggregate_literal_result_entry";
pub const TARGETS_INDEXED_SCALAR_RESULT_ENTRY: &str = "targets/indexed_scalar_result_entry";
pub const TARGETS_AARCH64_HFA_RESULT_ENTRY: &str = "targets/aarch64_hfa_result_entry";
pub const TARGETS_AARCH64_LARGE_RESULT_ENTRY: &str = "targets/aarch64_large_result_entry";
pub const TARGETS_SYSV_SMALL_AGGREGATE_ENTRY: &str = "targets/sysv_small_aggregate_entry";
pub const TARGETS_SYSV_ERASED_SMALL_AGGREGATE_ENTRY: &str =
    "targets/sysv_erased_small_aggregate_entry";
pub const TARGETS_SYSV_HFA_ENTRY_ARGUMENT: &str = "targets/sysv_hfa_entry_argument";
pub const TARGETS_SYSV_MIXED_AGGREGATE_ENTRY: &str = "targets/sysv_mixed_aggregate_entry";
pub const TARGETS_SYSV_MIXED_AGGREGATE_STACK_ENTRY: &str =
    "targets/sysv_mixed_aggregate_stack_entry";
pub const TARGETS_SYSV_SMALL_AGGREGATE_STACK_ENTRY: &str =
    "targets/sysv_small_aggregate_stack_entry";
pub const TARGETS_SYSV_LARGE_AGGREGATE_ENTRY: &str = "targets/sysv_large_aggregate_entry";
pub const TARGETS_SYSV_WIDE_AGGREGATE_ENTRY: &str = "targets/sysv_wide_aggregate_entry";
pub const TARGETS_SYSV_LARGE_RESULT_ENTRY: &str = "targets/sysv_large_result_entry";
pub const TARGETS_SYSV_LARGE_HFA_RESULT_ENTRY: &str = "targets/sysv_large_hfa_result_entry";
pub const TARGETS_SYSV_SMALL_RESULT_ENTRY: &str = "targets/sysv_small_result_entry";
pub const TARGETS_SYSV_HFA_RESULT_ENTRY: &str = "targets/sysv_hfa_result_entry";
pub const TARGETS_SYSV_MIXED_RESULT_ENTRY: &str = "targets/sysv_mixed_result_entry";
pub const TARGETS_SYSV_WRAPPED_FLOAT_ENTRY: &str = "targets/sysv_wrapped_float_entry";
pub const TARGETS_EFI_FREESTANDING_SKELETON: &str = "targets/efi_freestanding_skeleton";
pub const TARGETS_EFI_ENTRY_ARGUMENTS: &str = "targets/efi_entry_arguments";
pub const TARGETS_EFI_FLOAT_ENTRY_ARGUMENT: &str = "targets/efi_float_entry_argument";
pub const TARGETS_EFI_FLOAT_RESULT_ENTRY: &str = "targets/efi_float_result_entry";
pub const TARGETS_EFI_FLOAT_LITERAL_RESULT_ENTRY: &str = "targets/efi_float_literal_result_entry";
pub const TARGETS_EFI_U64_CONSTANT_RESULT_ENTRY: &str = "targets/efi_u64_constant_result_entry";
pub const TARGETS_EFI_SMALL_AGGREGATE_ENTRY: &str = "targets/efi_small_aggregate_entry";
pub const TARGETS_EFI_LARGE_RESULT_ENTRY: &str = "targets/efi_large_result_entry";
pub const TARGETS_EFI_LARGE_AGGREGATE_ENTRY: &str = "targets/efi_large_aggregate_entry";
pub const TARGETS_EFI_LARGE_AGGREGATE_STACK_ENTRY: &str = "targets/efi_large_aggregate_stack_entry";
pub const TARGETS_EFI_STACK_ENTRY_ARGUMENT: &str = "targets/efi_stack_entry_argument";
pub const TARGETS_ENTRY_RUN_ARGS_BYTES: &str = "targets/entry_run_args_bytes";
pub const TEXT_RUNTIME_UTF16_LITERAL_EXIT: &str = "text/runtime_utf16_literal_exit";
pub const COLLECTIONS_RUNTIME_CASE_ARRAY_ELEMENT_WRITE_EXIT: &str =
    "collections/runtime_case_array_element_write_exit";
pub const WIRE_RUNTIME_WIRE_POLICY_AUTHORED_PLAN_EXIT: &str =
    "wire/runtime_wire_policy_authored_plan_exit";
pub const WIRE_RUNTIME_WIRE_POLICY_AUTHORED_NESTED_EXIT: &str =
    "wire/runtime_wire_policy_authored_nested_exit";
pub const TARGETS_EFI_STRUCT_HANDOFF: &str = "targets/efi_struct_handoff";
pub const TARGETS_EFI_VTABLE_CALL: &str = "targets/efi_vtable_call";
pub const TARGETS_EFI_REF_PARAM_DIRECT_FACES: &str = "targets/efi_ref_param_direct_faces";
pub const TARGETS_EFI_REF_PARAM_CALL_ARG: &str = "targets/efi_ref_param_call_arg";
pub const CAPABILITIES_WIN64_SCALAR_FLOAT_IMPORT_COMPILE: &str =
    "capabilities/win64_scalar_float_import_compile";
pub const CAPABILITIES_WIN64_LARGE_AGGREGATE_IMPORT_COMPILE: &str =
    "capabilities/win64_large_aggregate_import_compile";
pub const CAPABILITIES_WIN64_DIRECT_AGGREGATE_IMPORT_COMPILE: &str =
    "capabilities/win64_direct_aggregate_import_compile";
pub const CAPABILITIES_WIN64_DIRECT_AGGREGATE_RESULT_IMPORT_COMPILE: &str =
    "capabilities/win64_direct_aggregate_result_import_compile";
pub const CAPABILITIES_WIN64_LARGE_AGGREGATE_RESULT_IMPORT_COMPILE: &str =
    "capabilities/win64_large_aggregate_result_import_compile";
pub const CAPABILITIES_SYSV_SMALL_AGGREGATE_IMPORT_COMPILE: &str =
    "capabilities/sysv_small_aggregate_import_compile";
pub const BUILD_STATIC_MACHINE_PARAMETER_CONFIG_COMPILE: &str =
    "build/static_machine_parameter_config_compile";
pub const INLINE_ASM_ASM_RUNTIME_PORT_MSR_FINAL_VALIDATION: &str =
    "inline_asm/asm_runtime_port_msr_final_validation";
pub const TEXT_RUNTIME_X86_GENERAL_DOUBLE_INDEXED_STRING_CONCAT_COMPILE: &str =
    "text/runtime_x86_general_double_indexed_string_concat_compile";
pub const SLICES_RUNTIME_AARCH64_CROSS_REGION_FRAME_INDEXED_RMW_COMPILE: &str =
    "slices/runtime_aarch64_cross_region_frame_indexed_rmw_compile";
pub const INLINE_ASM_ASM_MSR_COMPILE: &str = "inline_asm/asm_msr_compile";
pub const INLINE_ASM_ASM_CONTROL_REGISTERS_COMPILE: &str =
    "inline_asm/asm_control_registers_compile";
pub const INLINE_ASM_ASM_FLAGS_COMPILE: &str = "inline_asm/asm_flags_compile";

pub const PASS_CANARIES: &[&str] = &[
    BUILD_EXPLICIT_PROGRAM_ENTRY_BINDING,
    BUILD_UEFI_PROGRAM_ENTRY_STORAGE_ROOTS,
    ARITHMETIC_RUNTIME_CHAINED_FIELD_MUTATION_EXIT,
    INLINE_ASM_ASM_FENCES_COMPILE,
    TARGETS_AARCH64_HFA_ENTRY_ARGUMENT,
    TARGETS_AARCH64_SMALL_AGGREGATE_ENTRY,
    TARGETS_AARCH64_SMALL_AGGREGATE_STACK_ENTRY,
    TARGETS_AARCH64_LARGE_AGGREGATE_ENTRY,
    TARGETS_AARCH64_LARGE_AGGREGATE_STACK_ENTRY,
    TARGETS_AARCH64_WIDE_AGGREGATE_ENTRY,
    TARGETS_AARCH64_SMALL_RESULT_ENTRY,
    TARGETS_AGGREGATE_LITERAL_RESULT_ENTRY,
    TARGETS_INDEXED_SCALAR_RESULT_ENTRY,
    TARGETS_AARCH64_HFA_RESULT_ENTRY,
    TARGETS_AARCH64_LARGE_RESULT_ENTRY,
    TARGETS_SYSV_SMALL_AGGREGATE_ENTRY,
    TARGETS_SYSV_ERASED_SMALL_AGGREGATE_ENTRY,
    TARGETS_SYSV_HFA_ENTRY_ARGUMENT,
    TARGETS_SYSV_MIXED_AGGREGATE_ENTRY,
    TARGETS_SYSV_MIXED_AGGREGATE_STACK_ENTRY,
    TARGETS_SYSV_SMALL_AGGREGATE_STACK_ENTRY,
    TARGETS_SYSV_LARGE_AGGREGATE_ENTRY,
    TARGETS_SYSV_WIDE_AGGREGATE_ENTRY,
    TARGETS_SYSV_LARGE_RESULT_ENTRY,
    TARGETS_SYSV_LARGE_HFA_RESULT_ENTRY,
    TARGETS_SYSV_SMALL_RESULT_ENTRY,
    TARGETS_SYSV_HFA_RESULT_ENTRY,
    TARGETS_SYSV_MIXED_RESULT_ENTRY,
    TARGETS_SYSV_WRAPPED_FLOAT_ENTRY,
    TARGETS_EFI_FREESTANDING_SKELETON,
    TARGETS_EFI_ENTRY_ARGUMENTS,
    TARGETS_EFI_FLOAT_ENTRY_ARGUMENT,
    TARGETS_EFI_FLOAT_RESULT_ENTRY,
    TARGETS_EFI_FLOAT_LITERAL_RESULT_ENTRY,
    TARGETS_EFI_U64_CONSTANT_RESULT_ENTRY,
    TARGETS_EFI_SMALL_AGGREGATE_ENTRY,
    TARGETS_EFI_LARGE_RESULT_ENTRY,
    TARGETS_EFI_LARGE_AGGREGATE_ENTRY,
    TARGETS_EFI_LARGE_AGGREGATE_STACK_ENTRY,
    TARGETS_EFI_STACK_ENTRY_ARGUMENT,
    TARGETS_ENTRY_RUN_ARGS_BYTES,
    TEXT_RUNTIME_UTF16_LITERAL_EXIT,
    COLLECTIONS_RUNTIME_CASE_ARRAY_ELEMENT_WRITE_EXIT,
    WIRE_RUNTIME_WIRE_POLICY_AUTHORED_PLAN_EXIT,
    WIRE_RUNTIME_WIRE_POLICY_AUTHORED_NESTED_EXIT,
    TARGETS_EFI_STRUCT_HANDOFF,
    TARGETS_EFI_VTABLE_CALL,
    TARGETS_EFI_REF_PARAM_DIRECT_FACES,
    TARGETS_EFI_REF_PARAM_CALL_ARG,
];

pub const MIGRATED_ENTRY_PASS_CANARIES: &[(&str, &str)] = &[
    (
        CAPABILITIES_WIN64_SCALAR_FLOAT_IMPORT_COMPILE,
        "windows_x86_64",
    ),
    (
        CAPABILITIES_WIN64_LARGE_AGGREGATE_IMPORT_COMPILE,
        "windows_x86_64",
    ),
    (
        CAPABILITIES_WIN64_DIRECT_AGGREGATE_IMPORT_COMPILE,
        "windows_x86_64",
    ),
    (
        CAPABILITIES_WIN64_DIRECT_AGGREGATE_RESULT_IMPORT_COMPILE,
        "windows_x86_64",
    ),
    (
        CAPABILITIES_WIN64_LARGE_AGGREGATE_RESULT_IMPORT_COMPILE,
        "windows_x86_64",
    ),
    (
        CAPABILITIES_SYSV_SMALL_AGGREGATE_IMPORT_COMPILE,
        "linux_x86_64",
    ),
    (
        BUILD_STATIC_MACHINE_PARAMETER_CONFIG_COMPILE,
        "windows_x86_64",
    ),
    (INLINE_ASM_ASM_PORT_OUT_FINAL_VALIDATION, "linux_x86_64"),
    (
        INLINE_ASM_ASM_RUNTIME_PORT_MSR_FINAL_VALIDATION,
        "linux_x86_64",
    ),
    (
        TEXT_RUNTIME_X86_GENERAL_DOUBLE_INDEXED_STRING_CONCAT_COMPILE,
        "linux_x86_64",
    ),
    (
        SLICES_RUNTIME_AARCH64_CROSS_REGION_FRAME_INDEXED_RMW_COMPILE,
        "linux_arm64",
    ),
];

pub const MACHINE_CONTROL_PASS_CANARIES: &[(&str, i32)] = &[
    (INLINE_ASM_ASM_MSR_COMPILE, 2),
    (INLINE_ASM_ASM_CONTROL_REGISTERS_COMPILE, 7),
    (INLINE_ASM_ASM_FLAGS_COMPILE, 3),
    (INLINE_ASM_ASM_RUNTIME_PORT_MSR_FINAL_VALIDATION, 4),
];
