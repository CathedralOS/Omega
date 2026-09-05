//! Fixture identities shared with the executing owner and corpus inventory.

pub const RUNTIME_STDIN_COMMAND_BRANCH_EXIT: &str = "text/runtime_stdin_command_branch_exit";
pub const WINDOWS_WRAPPER_DARK_METHODS_EXIT: &str = "filesystem/windows_wrapper_dark_methods_exit";
pub const WINDOWS_WRAPPER_RESULTS_EXIT: &str = "filesystem/windows_wrapper_results_exit";
pub const RUNTIME_LOCAL_HOST_RESULT_DISPATCH_EXIT: &str =
    "filesystem/runtime_local_host_result_dispatch_exit";
pub const WINDOWS_RAW_ROUNDTRIP_EXIT: &str = "filesystem/windows_raw_roundtrip_exit";
pub const SELF_VALUE_CALL_LITERAL_PATH_EXIT: &str = "filesystem/self_value_call_literal_path_exit";
pub const DISCARDED_SELF_CALL_LITERAL_ERRNO_EXIT: &str =
    "filesystem/discarded_self_call_literal_errno_exit";
pub const WRAPPER_PARAM_SHADOW_EXIT: &str = "filesystem/wrapper_param_shadow_exit";
pub const WRAPPER_OPEN_WITH_EXIT: &str = "filesystem/wrapper_open_with_exit";
pub const FIELD_RECEIVER_METHOD_EXIT: &str = "filesystem/field_receiver_method_exit";
pub const RUNTIME_ARM_TARGET_HOST_RESULT_EXIT: &str = "calls/runtime_arm_target_host_result_exit";
pub const RUNTIME_QUALIFIED_CASE_VALUE_EXIT: &str = "expressions/runtime_qualified_case_value_exit";
pub const SINGLE_TARGET_INTERNAL_MACHINE_SKIPPED: &str =
    "targets/single_target_internal_machine_skipped";
pub const TARGET_MACHINE_GATING_EXIT: &str = "targets/target_machine_gating_exit";
pub const WINDOWS_WRAPPER_CREATE_NEW_EXIT: &str = "filesystem/windows_wrapper_create_new_exit";
pub const RING_REQUIREMENT_SATISFIES_EXIT: &str = "traits/ring_requirement_satisfies_exit";
pub const WINDOWS_FIND_ENUMERATION_EXIT: &str = "filesystem/windows_find_enumeration_exit";
pub const WINDOWS_READ_DIR_NTH_EXIT: &str = "filesystem/windows_read_dir_nth_exit";
pub const WINDOWS_SET_FILE_TIME_EXIT: &str = "filesystem/windows_set_file_time_exit";
pub const WINDOWS_WRAPPER_SET_TIMES_EXIT: &str = "filesystem/windows_wrapper_set_times_exit";
pub const WINDOWS_WRAPPER_LOCK_EXIT: &str = "filesystem/windows_wrapper_lock_exit";
pub const WINDOWS_CANONICALIZE_EXIT: &str = "filesystem/windows_canonicalize_exit";
pub const WINDOWS_HARD_LINK_EXIT: &str = "filesystem/windows_hard_link_exit";
pub const WINDOWS_POSITIONED_IO_EXIT: &str = "filesystem/windows_positioned_io_exit";
pub const WINDOWS_WRAPPER_METADATA_EXIT: &str = "filesystem/windows_wrapper_metadata_exit";
pub const WINDOWS_WRAPPER_EXISTS_EXIT: &str = "filesystem/windows_wrapper_exists_exit";
pub const WINDOWS_WRAPPER_SET_LEN_EXIT: &str = "filesystem/windows_wrapper_set_len_exit";
pub const WINDOWS_WRAPPER_COPY_EXIT: &str = "filesystem/windows_wrapper_copy_exit";
pub const AARCH64_STACK_IMPORT_COMPILE: &str = "capabilities/aarch64_stack_import_compile";
pub const NATIVE_FIXED_ARRAY_IMPORT_COMPILE: &str =
    "capabilities/native_fixed_array_import_compile";
pub const WIN64_POINTER_LENGTH_VS_DESCRIPTOR_COMPILE: &str =
    "capabilities/win64_pointer_length_vs_descriptor_compile";
pub const AARCH64_HFA_IMPORT_COMPILE: &str = "capabilities/aarch64_hfa_import_compile";
pub const AARCH64_ERASED_HFA_IMPORT_COMPILE: &str =
    "capabilities/aarch64_erased_hfa_import_compile";

pub const PASS_CANARIES: &[&str] = &[
    RUNTIME_STDIN_COMMAND_BRANCH_EXIT,
    WINDOWS_WRAPPER_DARK_METHODS_EXIT,
    WINDOWS_WRAPPER_RESULTS_EXIT,
    RUNTIME_LOCAL_HOST_RESULT_DISPATCH_EXIT,
    WINDOWS_RAW_ROUNDTRIP_EXIT,
    SELF_VALUE_CALL_LITERAL_PATH_EXIT,
    DISCARDED_SELF_CALL_LITERAL_ERRNO_EXIT,
    WRAPPER_PARAM_SHADOW_EXIT,
    WRAPPER_OPEN_WITH_EXIT,
    FIELD_RECEIVER_METHOD_EXIT,
    RUNTIME_ARM_TARGET_HOST_RESULT_EXIT,
    RUNTIME_QUALIFIED_CASE_VALUE_EXIT,
    SINGLE_TARGET_INTERNAL_MACHINE_SKIPPED,
    TARGET_MACHINE_GATING_EXIT,
    WINDOWS_WRAPPER_CREATE_NEW_EXIT,
    RING_REQUIREMENT_SATISFIES_EXIT,
    WINDOWS_FIND_ENUMERATION_EXIT,
    WINDOWS_READ_DIR_NTH_EXIT,
    WINDOWS_SET_FILE_TIME_EXIT,
    WINDOWS_WRAPPER_SET_TIMES_EXIT,
    WINDOWS_WRAPPER_LOCK_EXIT,
    WINDOWS_CANONICALIZE_EXIT,
    WINDOWS_HARD_LINK_EXIT,
    WINDOWS_POSITIONED_IO_EXIT,
    WINDOWS_WRAPPER_METADATA_EXIT,
    WINDOWS_WRAPPER_EXISTS_EXIT,
    WINDOWS_WRAPPER_SET_LEN_EXIT,
    WINDOWS_WRAPPER_COPY_EXIT,
    AARCH64_STACK_IMPORT_COMPILE,
    NATIVE_FIXED_ARRAY_IMPORT_COMPILE,
    WIN64_POINTER_LENGTH_VS_DESCRIPTOR_COMPILE,
    AARCH64_HFA_IMPORT_COMPILE,
    AARCH64_ERASED_HFA_IMPORT_COMPILE,
];

pub const CROSS_WINDOWS_PASS_CANARIES: &[&str] = &[
    "host/runtime_gui_window_lifecycle_exit",
    "host/runtime_user32_key_state_exit",
    "time/runtime_time_host_native_exit",
    "text/runtime_stdin_line_buffering_exit",
    "capabilities/windows_provides_import_exit",
];
