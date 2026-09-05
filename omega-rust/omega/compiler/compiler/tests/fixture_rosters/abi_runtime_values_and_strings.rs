//! Exact corpus inputs used by the ABI, runtime value, and string tests.
//! Execution stages, targets, input bytes, and assertions stay in the tests.

// The same fixture occurs in both short-path and repository-relative consumers.
macro_rules! repository_fixture {
    ($short:ident, $relative:ident, $path:literal) => {
        pub(crate) const $short: &str = $path;
        pub(crate) const $relative: &str = concat!("tests/omega/pass/", $path);
    };
}

pub(crate) const AARCH64_SCALAR_FLOAT_IMPORT_COMPILE: &str =
    "capabilities/aarch64_scalar_float_import_compile";
pub(crate) const AARCH64_SMALL_AGGREGATE_IMPORT_COMPILE: &str =
    "capabilities/aarch64_small_aggregate_import_compile";
pub(crate) const SYSV_SMALL_AGGREGATE_IMPORT_COMPILE: &str =
    "capabilities/sysv_small_aggregate_import_compile";
pub(crate) const AARCH64_SMALL_AGGREGATE_STACK_IMPORT_COMPILE: &str =
    "capabilities/aarch64_small_aggregate_stack_import_compile";
pub(crate) const AARCH64_HFA_STACK_IMPORT_COMPILE: &str =
    "capabilities/aarch64_hfa_stack_import_compile";
pub(crate) const AARCH64_HFA_RESULT_IMPORT_COMPILE: &str =
    "capabilities/aarch64_hfa_result_import_compile";
pub(crate) const AARCH64_ERASED_HFA_RESULT_IMPORT_COMPILE: &str =
    "capabilities/aarch64_erased_hfa_result_import_compile";
pub(crate) const AARCH64_SMALL_AGGREGATE_RESULT_IMPORT_COMPILE: &str =
    "capabilities/aarch64_small_aggregate_result_import_compile";
pub(crate) const AARCH64_LARGE_AGGREGATE_IMPORT_COMPILE: &str =
    "capabilities/aarch64_large_aggregate_import_compile";
pub(crate) const WIN64_LARGE_AGGREGATE_IMPORT_COMPILE: &str =
    "capabilities/win64_large_aggregate_import_compile";
pub(crate) const WIN64_DIRECT_AGGREGATE_IMPORT_COMPILE: &str =
    "capabilities/win64_direct_aggregate_import_compile";
pub(crate) const WIN64_DIRECT_AGGREGATE_RESULT_IMPORT_COMPILE: &str =
    "capabilities/win64_direct_aggregate_result_import_compile";
pub(crate) const WIN64_LARGE_AGGREGATE_RESULT_IMPORT_COMPILE: &str =
    "capabilities/win64_large_aggregate_result_import_compile";
pub(crate) const WIN64_SCALAR_FLOAT_IMPORT_COMPILE: &str =
    "capabilities/win64_scalar_float_import_compile";
pub(crate) const WINDOWS_PROVIDES_IMPORT_EXIT: &str = "capabilities/windows_provides_import_exit";
pub(crate) const WINDOWS_WRAPPER_BREADTH_EXIT: &str = "filesystem/windows_wrapper_breadth_exit";
pub(crate) const REPEATED_DIR_WALK_SCAN_EXIT: &str = "filesystem/repeated_dir_walk_scan_exit";
pub(crate) const WINDOWS_RAW_BREADTH_EXIT: &str = "filesystem/windows_raw_breadth_exit";
pub(crate) const RUNTIME_VALUE_CALL_ENTRY_FIELD_WRITE_EXIT: &str =
    "calls/runtime_value_call_entry_field_write_exit";
pub(crate) const RUNTIME_VALUE_CALLEE_POST_ENTRY_LETS_EXIT: &str =
    "calls/runtime_value_callee_post_entry_lets_exit";
pub(crate) const VALUE_MACHINE_SELF_ARRAY_LOCAL_INDEX_EXIT: &str =
    "backend/value_machine_self_array_local_index_exit";
pub(crate) const VALUE_MACHINE_CONST_INDEX_SELF_ARRAY_EXIT: &str =
    "backend/value_machine_const_index_self_array_exit";
pub(crate) const RUNTIME_POST_ENTRY_DEEP_CHAIN_EXIT: &str =
    "calls/runtime_post_entry_deep_chain_exit";
pub(crate) const RUNTIME_POST_ENTRY_CHAINED_LET_EXIT: &str =
    "calls/runtime_post_entry_chained_let_exit";
pub(crate) const RUNTIME_CROSS_CALLEE_DIVISION_EXIT: &str =
    "calls/runtime_cross_callee_division_exit";
pub(crate) const RUNTIME_CROSS_CALLEE_LET_NAMES_EXIT: &str =
    "calls/runtime_cross_callee_let_names_exit";
pub(crate) const RUNTIME_NESTED_VALUE_CALL_GUARD_EXIT: &str =
    "calls/runtime_nested_value_call_guard_exit";
pub(crate) const RUNTIME_TWO_SITE_STRUCT_RESULT_EXIT: &str =
    "calls/runtime_two_site_struct_result_exit";
pub(crate) const RUNTIME_VALUE_CALL_SAME_CALLEE_SITES_EXIT: &str =
    "calls/runtime_value_call_same_callee_sites_exit";
pub(crate) const RUNTIME_VALUE_CALL_TRANSITION_ARGS_EXIT: &str =
    "calls/runtime_value_call_transition_args_exit";
pub(crate) const RUNTIME_VALUE_CALL_TRANSITION_ARGS_STRAIGHT_LINE_EXIT: &str =
    "calls/runtime_value_call_transition_args_straight_line_exit";
pub(crate) const RUNTIME_VALUE_CALL_SHARED_SLOT_STRAIGHT_LINE_EXIT: &str =
    "calls/runtime_value_call_shared_slot_straight_line_exit";
pub(crate) const RUNTIME_ENUM_SELF_METHOD_EXIT: &str = "calls/runtime_enum_self_method_exit";
pub(crate) const RUNTIME_VALUE_CALL_DISPATCH_RESULTS_EXIT: &str =
    "calls/runtime_value_call_dispatch_results_exit";
pub(crate) const RUNTIME_VALUE_CALL_LITERAL_LEN_ARM_GUARD_EXIT: &str =
    "calls/runtime_value_call_literal_len_arm_guard_exit";
pub(crate) const RUNTIME_VALUE_CALL_GUARD_SUBJECT_EXIT: &str =
    "calls/runtime_value_call_guard_subject_exit";
pub(crate) const RUNTIME_EFFECTFUL_GUARD_LOCAL_AND_SELF_TERMINAL_EXIT: &str =
    "calls/runtime_effectful_guard_local_and_self_terminal_exit";
pub(crate) const RUNTIME_GUARDED_EFFECTFUL_TRANSITION_ARGUMENT_EXIT: &str =
    "calls/runtime_guarded_effectful_transition_argument_exit";
pub(crate) const RUNTIME_VALUE_CALL_NESTED_ENTRY_CALL_EXIT: &str =
    "calls/runtime_value_call_nested_entry_call_exit";
pub(crate) const RUNTIME_VALUE_CALL_SHARED_PAYLOAD_NAME_EXIT: &str =
    "calls/runtime_value_call_shared_payload_name_exit";
repository_fixture!(
    RUNTIME_VALUE_CALL_STRUCT_PAYLOAD_CAST_FIELD_EXIT,
    REPOSITORY_RUNTIME_VALUE_CALL_STRUCT_PAYLOAD_CAST_FIELD_EXIT,
    "calls/runtime_value_call_struct_payload_cast_field_exit"
);
pub(crate) const RUNTIME_BRANCH_LEAF_MULTIPLE_NAMED_CONVERSION_EXIT: &str =
    "calls/runtime_branch_leaf_multiple_named_conversion_exit";
pub(crate) const RUNTIME_STDIN_LINE_BUFFERING_EXIT: &str = "text/runtime_stdin_line_buffering_exit";
pub(crate) const RUNTIME_TEXT_STORAGE: &str = "text/runtime_text_storage";
pub(crate) const RUNTIME_STDERR_WRITE_EXIT: &str = "text/runtime_stderr_write_exit";
pub(crate) const RUNTIME_SLICE_ALIAS_INDEXED_STRING_FIELD_CONCAT_EXIT: &str =
    "text/runtime_slice_alias_indexed_string_field_concat_exit";
pub(crate) const RUNTIME_SLICE_INDEXED_STRING_GUARD_EXIT: &str =
    "text/runtime_slice_indexed_string_guard_exit";
pub(crate) const RUNTIME_SLICE_MACHINE_INDEXED_STRING_GUARD_EXIT: &str =
    "text/runtime_slice_machine_indexed_string_guard_exit";
pub(crate) const RUNTIME_STRING_FIELD_LITERAL_GUARD_EXIT: &str =
    "text/runtime_string_field_literal_guard_exit";
pub(crate) const RUNTIME_LOCAL_ARRAY_INDEXED_STRING_GUARD_EXIT: &str =
    "text/runtime_local_array_indexed_string_guard_exit";
pub(crate) const RUNTIME_LOCAL_ARRAY_INDEXED_STRING_FIELD_CONCAT_EXIT: &str =
    "text/runtime_local_array_indexed_string_field_concat_exit";
pub(crate) const RUNTIME_SLICE_FIXED_INDEXED_STRING_GUARD_EXIT: &str =
    "text/runtime_slice_fixed_indexed_string_guard_exit";
pub(crate) const RUNTIME_POINTEE_STRING_GUARD_EXIT: &str = "text/runtime_pointee_string_guard_exit";
pub(crate) const RUNTIME_MUTABLE_STRING_PARAMETER_CONCAT_EXIT: &str =
    "text/runtime_mutable_string_parameter_concat_exit";
pub(crate) const RUNTIME_MUTABLE_STRING_PARAMETER_CONCAT_WRITE_LINE: &str =
    "text/runtime_mutable_string_parameter_concat_write_line";
pub(crate) const RUNTIME_MUTABLE_STRING_PARAMETER_WRAPPED_CONCAT_WRITE_LINE: &str =
    "text/runtime_mutable_string_parameter_wrapped_concat_write_line";
pub(crate) const RUNTIME_MUTABLE_STRUCT_STRING_FIELD_COPY_CONCAT_EXIT: &str =
    "text/runtime_mutable_struct_string_field_copy_concat_exit";
pub(crate) const RUNTIME_LOCAL_STRUCT_STRING_FIELD_CONCAT_EXIT: &str =
    "text/runtime_local_struct_string_field_concat_exit";
pub(crate) const RUNTIME_STRING_STORED_SUFFIX_EXIT: &str = "text/runtime_string_stored_suffix_exit";
pub(crate) const RUNTIME_LOOKUP_STRUCT_FIELD_CONCAT_EXIT: &str =
    "text/runtime_lookup_struct_field_concat_exit";
pub(crate) const RUNTIME_LARGE_LOOKUP_STRUCT_FIELD_CONCAT_EXIT: &str =
    "text/runtime_large_lookup_struct_field_concat_exit";
pub(crate) const RUNTIME_LARGE_ROOM_LOOKUP_STRUCT_FIELD_CONCAT_EXIT: &str =
    "text/runtime_large_room_lookup_struct_field_concat_exit";
pub(crate) const RUNTIME_CALL_ARGUMENT_STRUCT_STRING_FIELD_SLICE_ALIAS_EXIT: &str =
    "text/runtime_call_argument_struct_string_field_slice_alias_exit";
pub(crate) const RUNTIME_MUTABLE_STRUCT_STRING_FIELD_COPY_CONCAT_WRITE_LINE: &str =
    "text/runtime_mutable_struct_string_field_copy_concat_write_line";
pub(crate) const RUNTIME_MACHINE_OWNED_INDEXED_INTEGER_WRITE_EXIT: &str =
    "storage/runtime_machine_owned_indexed_integer_write_exit";
pub(crate) const RUNTIME_MACHINE_OWNED_FIXED_INDEXED_STRUCT_COPY_EXIT: &str =
    "storage/runtime_machine_owned_fixed_indexed_struct_copy_exit";
pub(crate) const RUNTIME_MACHINE_OWNED_INDEXED_STRUCT_COPY_EXIT: &str =
    "storage/runtime_machine_owned_indexed_struct_copy_exit";
pub(crate) const RUNTIME_MACHINE_OWNED_INDEXED_NESTED_EXIT_WRITE_EXIT: &str =
    "storage/runtime_machine_owned_indexed_nested_exit_write_exit";
pub(crate) const RUNTIME_ORDERED_ROOM_DISPATCH_EXIT: &str =
    "dungeon/runtime_ordered_room_dispatch_exit";
pub(crate) const RUNTIME_ORDERED_ROOM_DISPATCH_AFTER_CALL_EXIT: &str =
    "dungeon/runtime_ordered_room_dispatch_after_call_exit";
pub(crate) const RUNTIME_ORDERED_ROOM_DISPATCH_GAME_SHAPE_EXIT: &str =
    "dungeon/runtime_ordered_room_dispatch_game_shape_exit";
pub(crate) const RUNTIME_ORDERED_ROOM_DISPATCH_LARGE_MACHINE_EXIT: &str =
    "dungeon/runtime_ordered_room_dispatch_large_machine_exit";
pub(crate) const RUNTIME_ORDERED_ROOM_DISPATCH_LOOP_EXIT: &str =
    "dungeon/runtime_ordered_room_dispatch_loop_exit";
pub(crate) const RUNTIME_GUARDED_INLINE_LEAF_ARM_SKIP_EXIT: &str =
    "dungeon/runtime_guarded_inline_leaf_arm_skip_exit";
pub(crate) const RUNTIME_ORDERED_ROOM_DISPATCH_REAL_SHOW_STATES_EXIT: &str =
    "dungeon/runtime_ordered_room_dispatch_real_show_states_exit";
pub(crate) const RUNTIME_THREADED_MUT_ARG_INTERRUPT_SOAK_EXIT: &str =
    "dungeon/runtime_threaded_mut_arg_interrupt_soak_exit";
pub(crate) const WINDOWS_POSITIONED_IO_EXIT: &str = "filesystem/windows_positioned_io_exit";
repository_fixture!(
    RUNTIME_NESTED_VALUE_CALL_CALLER_LOCAL_GUARD_EXIT,
    REPOSITORY_RUNTIME_NESTED_VALUE_CALL_CALLER_LOCAL_GUARD_EXIT,
    "dungeon/runtime_nested_value_call_caller_local_guard_exit"
);
pub(crate) const RUNTIME_CLEAR_CARVE_RENDER_STRING_FIELDS_EXIT: &str =
    "dungeon/runtime_clear_carve_render_string_fields_exit";
pub(crate) const RUNTIME_FULL_LEVEL_WRAPPER_LOOKUP_STRING_FIELD_EXIT: &str =
    "dungeon/runtime_full_level_wrapper_lookup_string_field_exit";
pub(crate) const MUTABLE_OUTPUT_HOST_CALL: &str = "calls/mutable_output_host_call";
pub(crate) const RUNTIME_STDIN_COMMAND_BRANCH_EXIT: &str = "text/runtime_stdin_command_branch_exit";
pub(crate) const RUNTIME_CHAINED_STRING_APPEND_EXIT: &str =
    "text/runtime_chained_string_append_exit";
pub(crate) const RUNTIME_MACHINE_STRING_APPEND_IN_PLACE_EXIT: &str =
    "text/runtime_machine_string_append_in_place_exit";
pub(crate) const REPOSITORY_RUNTIME_CONTAINED_RANGE_WRITE: &str =
    "tests/omega/pass/arithmetic/runtime_contained_range_write";
pub(crate) const REPOSITORY_RUNTIME_UNSIGNED_MODULO_CALL_ARGUMENT_EXIT: &str =
    "tests/omega/pass/arithmetic/runtime_unsigned_modulo_call_argument_exit";
pub(crate) const REPOSITORY_RUNTIME_CALL_ENUM_SEQUENCE: &str =
    "tests/omega/pass/calls/runtime_call_enum_sequence";
pub(crate) const REPOSITORY_RUNTIME_NESTED_NAMED_CONVERSION_ALIAS_EXIT: &str =
    "tests/omega/pass/calls/runtime_nested_named_conversion_alias_exit";
pub(crate) const REPOSITORY_RUNTIME_BRANCHING_HELPER_LOCAL_GUARD_VALUE: &str =
    "tests/omega/pass/control_flow/runtime_branching_helper_local_guard_value";
pub(crate) const REPOSITORY_RUNTIME_CONTAINED_REWARD_TABLE_ROLL_ITEM: &str =
    "tests/omega/pass/rewards/runtime_contained_reward_table_roll_item";
pub(crate) const REPOSITORY_RUNTIME_REWARD_TABLE_ROLL_ITEM_SHAPE: &str =
    "tests/omega/pass/rewards/runtime_reward_table_roll_item_shape";
pub(crate) const REPOSITORY_NATIVE_COPY_PRESERVE: &str =
    "tests/omega/pass/filesystem/native_copy_preserve";
pub(crate) const REPOSITORY_NATIVE_FILETYPE: &str = "tests/omega/pass/filesystem/native_filetype";
pub(crate) const REPOSITORY_NATIVE_FS_WORKFLOW: &str =
    "tests/omega/pass/filesystem/native_fs_workflow";
pub(crate) const REPOSITORY_NATIVE_FSTAT: &str = "tests/omega/pass/filesystem/native_fstat";
pub(crate) const REPOSITORY_NATIVE_METADATA_BLOCKS: &str =
    "tests/omega/pass/filesystem/native_metadata_blocks";
pub(crate) const REPOSITORY_NATIVE_METADATA_CTIME_DEV: &str =
    "tests/omega/pass/filesystem/native_metadata_ctime_dev";
pub(crate) const REPOSITORY_NATIVE_METADATA_INO: &str =
    "tests/omega/pass/filesystem/native_metadata_ino";
pub(crate) const REPOSITORY_NATIVE_METADATA_MODIFIED: &str =
    "tests/omega/pass/filesystem/native_metadata_modified";
pub(crate) const REPOSITORY_NATIVE_METADATA_NLINK: &str =
    "tests/omega/pass/filesystem/native_metadata_nlink";
pub(crate) const REPOSITORY_NATIVE_METADATA_READONLY: &str =
    "tests/omega/pass/filesystem/native_metadata_readonly";
pub(crate) const REPOSITORY_NATIVE_METADATA_TIMES: &str =
    "tests/omega/pass/filesystem/native_metadata_times";
pub(crate) const REPOSITORY_NATIVE_OPEN_CREATE: &str =
    "tests/omega/pass/filesystem/native_open_create";
pub(crate) const REPOSITORY_NATIVE_SET_TIMES: &str = "tests/omega/pass/filesystem/native_set_times";
pub(crate) const REPOSITORY_NATIVE_STAT: &str = "tests/omega/pass/filesystem/native_stat";
pub(crate) const REPOSITORY_NATIVE_SYMLINK_METADATA: &str =
    "tests/omega/pass/filesystem/native_symlink_metadata";
pub(crate) const REPOSITORY_RUNTIME_FS_MTIME_INTEROP_WINDOWS_EXIT: &str =
    "tests/omega/pass/time/runtime_fs_mtime_interop_windows_exit";
pub(crate) const REPOSITORY_RUNTIME_FS_MTIME_SYSTEM_TIME_INTEROP_EXIT: &str =
    "tests/omega/pass/time/runtime_fs_mtime_system_time_interop_exit";
pub(crate) const REPOSITORY_WINDOWS_SET_FILE_TIME_EXIT: &str =
    "tests/omega/pass/filesystem/windows_set_file_time_exit";

pub(crate) const PASS_CANARIES: &[&str] = &[
    AARCH64_SCALAR_FLOAT_IMPORT_COMPILE,
    AARCH64_SMALL_AGGREGATE_IMPORT_COMPILE,
    SYSV_SMALL_AGGREGATE_IMPORT_COMPILE,
    AARCH64_SMALL_AGGREGATE_STACK_IMPORT_COMPILE,
    AARCH64_HFA_STACK_IMPORT_COMPILE,
    AARCH64_HFA_RESULT_IMPORT_COMPILE,
    AARCH64_ERASED_HFA_RESULT_IMPORT_COMPILE,
    AARCH64_SMALL_AGGREGATE_RESULT_IMPORT_COMPILE,
    AARCH64_LARGE_AGGREGATE_IMPORT_COMPILE,
    WIN64_LARGE_AGGREGATE_IMPORT_COMPILE,
    WIN64_DIRECT_AGGREGATE_IMPORT_COMPILE,
    WIN64_DIRECT_AGGREGATE_RESULT_IMPORT_COMPILE,
    WIN64_LARGE_AGGREGATE_RESULT_IMPORT_COMPILE,
    WIN64_SCALAR_FLOAT_IMPORT_COMPILE,
    WINDOWS_PROVIDES_IMPORT_EXIT,
    WINDOWS_WRAPPER_BREADTH_EXIT,
    REPEATED_DIR_WALK_SCAN_EXIT,
    WINDOWS_RAW_BREADTH_EXIT,
    RUNTIME_VALUE_CALL_ENTRY_FIELD_WRITE_EXIT,
    RUNTIME_VALUE_CALLEE_POST_ENTRY_LETS_EXIT,
    VALUE_MACHINE_SELF_ARRAY_LOCAL_INDEX_EXIT,
    VALUE_MACHINE_CONST_INDEX_SELF_ARRAY_EXIT,
    RUNTIME_POST_ENTRY_DEEP_CHAIN_EXIT,
    RUNTIME_POST_ENTRY_CHAINED_LET_EXIT,
    RUNTIME_CROSS_CALLEE_DIVISION_EXIT,
    RUNTIME_CROSS_CALLEE_LET_NAMES_EXIT,
    RUNTIME_NESTED_VALUE_CALL_GUARD_EXIT,
    RUNTIME_TWO_SITE_STRUCT_RESULT_EXIT,
    RUNTIME_VALUE_CALL_SAME_CALLEE_SITES_EXIT,
    RUNTIME_VALUE_CALL_TRANSITION_ARGS_EXIT,
    RUNTIME_VALUE_CALL_TRANSITION_ARGS_STRAIGHT_LINE_EXIT,
    RUNTIME_VALUE_CALL_SHARED_SLOT_STRAIGHT_LINE_EXIT,
    RUNTIME_ENUM_SELF_METHOD_EXIT,
    RUNTIME_VALUE_CALL_DISPATCH_RESULTS_EXIT,
    RUNTIME_VALUE_CALL_LITERAL_LEN_ARM_GUARD_EXIT,
    RUNTIME_VALUE_CALL_GUARD_SUBJECT_EXIT,
    RUNTIME_EFFECTFUL_GUARD_LOCAL_AND_SELF_TERMINAL_EXIT,
    RUNTIME_GUARDED_EFFECTFUL_TRANSITION_ARGUMENT_EXIT,
    RUNTIME_VALUE_CALL_NESTED_ENTRY_CALL_EXIT,
    RUNTIME_VALUE_CALL_SHARED_PAYLOAD_NAME_EXIT,
    RUNTIME_BRANCH_LEAF_MULTIPLE_NAMED_CONVERSION_EXIT,
    RUNTIME_STDERR_WRITE_EXIT,
    RUNTIME_SLICE_ALIAS_INDEXED_STRING_FIELD_CONCAT_EXIT,
    RUNTIME_SLICE_INDEXED_STRING_GUARD_EXIT,
    RUNTIME_SLICE_MACHINE_INDEXED_STRING_GUARD_EXIT,
    RUNTIME_STRING_FIELD_LITERAL_GUARD_EXIT,
    RUNTIME_LOCAL_ARRAY_INDEXED_STRING_GUARD_EXIT,
    RUNTIME_LOCAL_ARRAY_INDEXED_STRING_FIELD_CONCAT_EXIT,
    RUNTIME_SLICE_FIXED_INDEXED_STRING_GUARD_EXIT,
    RUNTIME_POINTEE_STRING_GUARD_EXIT,
    RUNTIME_LOCAL_STRUCT_STRING_FIELD_CONCAT_EXIT,
    RUNTIME_STRING_STORED_SUFFIX_EXIT,
    RUNTIME_LOOKUP_STRUCT_FIELD_CONCAT_EXIT,
    RUNTIME_LARGE_LOOKUP_STRUCT_FIELD_CONCAT_EXIT,
    RUNTIME_LARGE_ROOM_LOOKUP_STRUCT_FIELD_CONCAT_EXIT,
    RUNTIME_CALL_ARGUMENT_STRUCT_STRING_FIELD_SLICE_ALIAS_EXIT,
    RUNTIME_MACHINE_OWNED_INDEXED_INTEGER_WRITE_EXIT,
    RUNTIME_MACHINE_OWNED_FIXED_INDEXED_STRUCT_COPY_EXIT,
    RUNTIME_MACHINE_OWNED_INDEXED_STRUCT_COPY_EXIT,
    RUNTIME_MACHINE_OWNED_INDEXED_NESTED_EXIT_WRITE_EXIT,
    RUNTIME_ORDERED_ROOM_DISPATCH_EXIT,
    RUNTIME_ORDERED_ROOM_DISPATCH_AFTER_CALL_EXIT,
    RUNTIME_ORDERED_ROOM_DISPATCH_GAME_SHAPE_EXIT,
    RUNTIME_GUARDED_INLINE_LEAF_ARM_SKIP_EXIT,
    RUNTIME_THREADED_MUT_ARG_INTERRUPT_SOAK_EXIT,
    WINDOWS_POSITIONED_IO_EXIT,
];

pub(crate) const BOUNDED_CARRIER_PASS_CANARIES: &[&str] = &[
    RUNTIME_MUTABLE_STRING_PARAMETER_CONCAT_EXIT,
    RUNTIME_MUTABLE_STRUCT_STRING_FIELD_COPY_CONCAT_EXIT,
    RUNTIME_MUTABLE_STRING_PARAMETER_CONCAT_WRITE_LINE,
    RUNTIME_MUTABLE_STRING_PARAMETER_WRAPPED_CONCAT_WRITE_LINE,
    RUNTIME_MUTABLE_STRUCT_STRING_FIELD_COPY_CONCAT_WRITE_LINE,
    RUNTIME_CLEAR_CARVE_RENDER_STRING_FIELDS_EXIT,
    RUNTIME_FULL_LEVEL_WRAPPER_LOOKUP_STRING_FIELD_EXIT,
    MUTABLE_OUTPUT_HOST_CALL,
    RUNTIME_TEXT_STORAGE,
    RUNTIME_STDIN_LINE_BUFFERING_EXIT,
    RUNTIME_STDIN_COMMAND_BRANCH_EXIT,
    RUNTIME_ORDERED_ROOM_DISPATCH_LOOP_EXIT,
    RUNTIME_ORDERED_ROOM_DISPATCH_LARGE_MACHINE_EXIT,
    RUNTIME_ORDERED_ROOM_DISPATCH_REAL_SHOW_STATES_EXIT,
    RUNTIME_CHAINED_STRING_APPEND_EXIT,
    RUNTIME_MACHINE_STRING_APPEND_IN_PLACE_EXIT,
];

pub(crate) const PRNG_REPOSITORY_PASS_CANARIES: &[&str] = &[
    REPOSITORY_RUNTIME_CONTAINED_RANGE_WRITE,
    REPOSITORY_RUNTIME_UNSIGNED_MODULO_CALL_ARGUMENT_EXIT,
    REPOSITORY_RUNTIME_CALL_ENUM_SEQUENCE,
    REPOSITORY_RUNTIME_NESTED_NAMED_CONVERSION_ALIAS_EXIT,
    REPOSITORY_RUNTIME_BRANCHING_HELPER_LOCAL_GUARD_VALUE,
    REPOSITORY_RUNTIME_NESTED_VALUE_CALL_CALLER_LOCAL_GUARD_EXIT,
    REPOSITORY_RUNTIME_CONTAINED_REWARD_TABLE_ROLL_ITEM,
    REPOSITORY_RUNTIME_REWARD_TABLE_ROLL_ITEM_SHAPE,
];

pub(crate) const FILESYSTEM_REPOSITORY_PASS_CANARIES: &[&str] = &[
    REPOSITORY_RUNTIME_VALUE_CALL_STRUCT_PAYLOAD_CAST_FIELD_EXIT,
    REPOSITORY_NATIVE_COPY_PRESERVE,
    REPOSITORY_NATIVE_FILETYPE,
    REPOSITORY_NATIVE_FS_WORKFLOW,
    REPOSITORY_NATIVE_FSTAT,
    REPOSITORY_NATIVE_METADATA_BLOCKS,
    REPOSITORY_NATIVE_METADATA_CTIME_DEV,
    REPOSITORY_NATIVE_METADATA_INO,
    REPOSITORY_NATIVE_METADATA_MODIFIED,
    REPOSITORY_NATIVE_METADATA_NLINK,
    REPOSITORY_NATIVE_METADATA_READONLY,
    REPOSITORY_NATIVE_METADATA_TIMES,
    REPOSITORY_NATIVE_OPEN_CREATE,
    REPOSITORY_NATIVE_SET_TIMES,
    REPOSITORY_NATIVE_STAT,
    REPOSITORY_NATIVE_SYMLINK_METADATA,
    REPOSITORY_RUNTIME_FS_MTIME_INTEROP_WINDOWS_EXIT,
    REPOSITORY_RUNTIME_FS_MTIME_SYSTEM_TIME_INTEROP_EXIT,
];

pub(crate) const REPOSITORY_PASS_CANARIES: &[&str] = &[REPOSITORY_WINDOWS_SET_FILE_TIME_EXIT];
