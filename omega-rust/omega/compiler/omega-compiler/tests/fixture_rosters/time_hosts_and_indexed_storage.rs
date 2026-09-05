//! Corpus inputs used by the time, host, and indexed-storage tests.
//! Targets, staging, execution, and inline failure assertions stay with their owners.

pub(crate) const RUNTIME_I64_MIN_LITERAL_EXIT: &str = "arithmetic/runtime_i64_min_literal_exit";
pub(crate) const RUNTIME_TIME_HOST_VIRTUAL_EXIT: &str = "time/runtime_time_host_virtual_exit";
pub(crate) const RUNTIME_TIME_ELAPSED_SINCE_EXIT: &str = "time/runtime_time_elapsed_since_exit";
pub(crate) const CROSS_DARWIN_TIME_HOST: &str = "time/cross_darwin_time_host";
pub(crate) const RUNTIME_TICK_COUNT_MONOTONIC_EXIT: &str = "host/runtime_tick_count_monotonic_exit";
pub(crate) const NATIVE_OPEN_CREATE: &str = "filesystem/native_open_create";
pub(crate) const NATIVE_FLOAT_TWO_ARGS: &str = "float/native_float_two_args";
pub(crate) const NATIVE_ERRNO: &str = "filesystem/native_errno";
pub(crate) const CROSS_LINUX_TIME_HOST: &str = "time/cross_linux_time_host";
pub(crate) const CROSS_LINUX_VALUE_SYSCALLS: &str = "filesystem/cross_linux_value_syscalls";
pub(crate) const RUNTIME_CHECKED_TIME_ARITH_EXIT: &str = "time/runtime_checked_time_arith_exit";
pub(crate) const RUNTIME_SLEEP_FOR_EXIT: &str = "time/runtime_sleep_for_exit";
pub(crate) const RUNTIME_SYSTEM_TIME_AFTER_2026_EXIT: &str =
    "time/runtime_system_time_after_2026_exit";
pub(crate) const RUNTIME_INSTANT_ELAPSED_EXIT: &str = "time/runtime_instant_elapsed_exit";
pub(crate) const RUNTIME_TIME_HOST_NATIVE_EXIT: &str = "time/runtime_time_host_native_exit";
pub(crate) const RUNTIME_TIME_HOST_NATIVE_DARWIN_EXIT: &str =
    "time/runtime_time_host_native_darwin_exit";
pub(crate) const RUNTIME_FS_MTIME_SYSTEM_TIME_INTEROP_EXIT: &str =
    "time/runtime_fs_mtime_system_time_interop_exit";
pub(crate) const RUNTIME_FS_MTIME_INTEROP_WINDOWS_EXIT: &str =
    "time/runtime_fs_mtime_interop_windows_exit";
pub(crate) const RUNTIME_DURATION_TOTALS_EXIT: &str = "time/runtime_duration_totals_exit";
pub(crate) const RUNTIME_DURATION_CONSTRUCTORS_EXIT: &str =
    "time/runtime_duration_constructors_exit";
pub(crate) const RUNTIME_DURATION_CORE_EXIT: &str = "time/runtime_duration_core_exit";
pub(crate) const RUNTIME_SCOPED_CONST_EXIT: &str = "constants/runtime_scoped_const_exit";
pub(crate) const RUNTIME_U64_MAX_LITERAL_EXIT: &str = "arithmetic/runtime_u64_max_literal_exit";
pub(crate) const RUNTIME_GUARDED_COMPUTED_INDEX_OPERAND_EXIT: &str =
    "collections/runtime_guarded_computed_index_operand_exit";
pub(crate) const RUNTIME_COMPUTED_INDEX_DIRECT_EXIT: &str =
    "collections/runtime_computed_index_direct_exit";
pub(crate) const RUNTIME_DUAL_INDEXED_COPY_EXIT: &str =
    "collections/runtime_dual_indexed_copy_exit";
pub(crate) const RUNTIME_DOUBLE_INDEXED_WRITE_EXIT: &str =
    "collections/runtime_double_indexed_write_exit";
pub(crate) const RUNTIME_CONTAINER_SETTER_MATRIX_EXIT: &str =
    "generics/runtime_container_setter_matrix_exit";
pub(crate) const RUNTIME_CONTAINER_METHOD_INSTANCES_EXIT: &str =
    "generics/runtime_container_method_instances_exit";
pub(crate) const RUNTIME_FRAME_DOUBLE_INDEXED_READ_EXIT: &str =
    "collections/runtime_frame_double_indexed_read_exit";
pub(crate) const RUNTIME_DOUBLE_INDEXED_RMW_EXIT: &str =
    "collections/runtime_double_indexed_rmw_exit";
pub(crate) const RUNTIME_INDEXED_OPERAND_TRANSITION_ARG_EXIT: &str =
    "collections/runtime_indexed_operand_transition_arg_exit";
pub(crate) const RUNTIME_SHARED_REF_PARAM_GUARD_EXIT: &str =
    "references/runtime_shared_ref_param_guard_exit";
pub(crate) const RUNTIME_NESTED_RECEIVER_DISTINCT_TYPES_EXIT: &str =
    "references/runtime_nested_receiver_distinct_types_exit";
pub(crate) const RUNTIME_DOUBLE_INDEXED_MEMBER_EXIT: &str =
    "collections/runtime_double_indexed_member_exit";
pub(crate) const RUNTIME_DOUBLE_INDEXED_OPERAND_EXIT: &str =
    "collections/runtime_double_indexed_operand_exit";
pub(crate) const RUNTIME_INPLACE_REVERSE_LOCAL_TEMP_EXIT: &str =
    "collections/runtime_inplace_reverse_local_temp_exit";
pub(crate) const RUNTIME_INDEXED_LOCAL_COPY_CHAIN_EXIT: &str =
    "collections/runtime_indexed_local_copy_chain_exit";
pub(crate) const RUNTIME_INDEXED_WRITE_FRAME_LOCAL_SOURCE_EXIT: &str =
    "collections/runtime_indexed_write_frame_local_source_exit";
pub(crate) const RUNTIME_CAPTURED_LOCAL_SWAP_EXIT: &str =
    "control_flow/runtime_captured_local_swap_exit";
pub(crate) const RUNTIME_DUAL_INDEXED_COPY_IN_LOOP_EXIT: &str =
    "collections/runtime_dual_indexed_copy_in_loop_exit";
pub(crate) const RUNTIME_GUI_FOREGROUND_WINDOW_EXIT: &str =
    "host/runtime_gui_foreground_window_exit";
pub(crate) const NATIVE_CLOSE: &str = "filesystem/native_close";
pub(crate) const RUNTIME_IMPORT_CALL_ARGUMENT_EXIT: &str =
    "providers/runtime_import_call_argument_exit";
pub(crate) const WINDOWS_PROVIDES_IMPORT_EXIT: &str = "capabilities/windows_provides_import_exit";
pub(crate) const U64_LITERAL_ABOVE_I64_MAX: &str = "arithmetic/u64_literal_above_i64_max";

pub(crate) const STORAGE_RESULT_IMPORT_CANARIES: &[(&str, &str)] = &[
    ("windows_x86_64", RUNTIME_GUI_FOREGROUND_WINDOW_EXIT),
    ("macos_arm64", NATIVE_CLOSE),
];

pub(crate) const AUTHORED_SCALAR_IMPORT_CANARIES: &[(&str, &str)] = &[
    ("macos_arm64", RUNTIME_IMPORT_CALL_ARGUMENT_EXIT),
    ("windows_x86_64", WINDOWS_PROVIDES_IMPORT_EXIT),
];

pub(crate) const PASS_CANARIES: &[&str] = &[
    RUNTIME_I64_MIN_LITERAL_EXIT,
    RUNTIME_TIME_HOST_VIRTUAL_EXIT,
    RUNTIME_TIME_ELAPSED_SINCE_EXIT,
    CROSS_DARWIN_TIME_HOST,
    RUNTIME_TICK_COUNT_MONOTONIC_EXIT,
    NATIVE_OPEN_CREATE,
    NATIVE_FLOAT_TWO_ARGS,
    NATIVE_ERRNO,
    CROSS_LINUX_TIME_HOST,
    CROSS_LINUX_VALUE_SYSCALLS,
    RUNTIME_CHECKED_TIME_ARITH_EXIT,
    RUNTIME_SLEEP_FOR_EXIT,
    RUNTIME_SYSTEM_TIME_AFTER_2026_EXIT,
    RUNTIME_INSTANT_ELAPSED_EXIT,
    RUNTIME_TIME_HOST_NATIVE_EXIT,
    RUNTIME_TIME_HOST_NATIVE_DARWIN_EXIT,
    RUNTIME_FS_MTIME_SYSTEM_TIME_INTEROP_EXIT,
    RUNTIME_FS_MTIME_INTEROP_WINDOWS_EXIT,
    RUNTIME_DURATION_TOTALS_EXIT,
    RUNTIME_DURATION_CONSTRUCTORS_EXIT,
    RUNTIME_DURATION_CORE_EXIT,
    RUNTIME_SCOPED_CONST_EXIT,
    RUNTIME_U64_MAX_LITERAL_EXIT,
    RUNTIME_GUARDED_COMPUTED_INDEX_OPERAND_EXIT,
    RUNTIME_COMPUTED_INDEX_DIRECT_EXIT,
    RUNTIME_DUAL_INDEXED_COPY_EXIT,
    RUNTIME_DOUBLE_INDEXED_WRITE_EXIT,
    RUNTIME_CONTAINER_SETTER_MATRIX_EXIT,
    RUNTIME_CONTAINER_METHOD_INSTANCES_EXIT,
    RUNTIME_FRAME_DOUBLE_INDEXED_READ_EXIT,
    RUNTIME_DOUBLE_INDEXED_RMW_EXIT,
    RUNTIME_INDEXED_OPERAND_TRANSITION_ARG_EXIT,
    RUNTIME_SHARED_REF_PARAM_GUARD_EXIT,
    RUNTIME_NESTED_RECEIVER_DISTINCT_TYPES_EXIT,
    RUNTIME_DOUBLE_INDEXED_MEMBER_EXIT,
    RUNTIME_DOUBLE_INDEXED_OPERAND_EXIT,
    RUNTIME_INPLACE_REVERSE_LOCAL_TEMP_EXIT,
    RUNTIME_INDEXED_LOCAL_COPY_CHAIN_EXIT,
    RUNTIME_INDEXED_WRITE_FRAME_LOCAL_SOURCE_EXIT,
    RUNTIME_CAPTURED_LOCAL_SWAP_EXIT,
    RUNTIME_DUAL_INDEXED_COPY_IN_LOOP_EXIT,
];

pub(crate) const FAIL_CANARIES: &[&str] = &[U64_LITERAL_ABOVE_I64_MAX];
