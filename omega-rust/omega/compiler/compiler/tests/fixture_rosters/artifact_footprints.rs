//! Fixture identities shared with the executing owner and corpus inventory.

pub const RUNTIME_INTEGER_LITERAL_DISPATCH_EXIT: &str =
    "control_flow/runtime_integer_literal_dispatch_exit";
pub const RUNTIME_LOCAL_STRUCT_STRING_FIELD_CONCAT_EXIT: &str =
    "text/runtime_local_struct_string_field_concat_exit";
pub const TERMINATION_INDEX_DISTANCE_COMPILE: &str =
    "control_flow/termination_index_distance_compile";
pub const RUNTIME_VALUE_CALL_THROUGH_ALIAS_IN_DISPATCH_EXIT: &str =
    "calls/runtime_value_call_through_alias_in_dispatch_exit";
pub const RUNTIME_SHARED_REF_PARAM_COPY_EXIT: &str = "calls/runtime_shared_ref_param_copy_exit";
pub const RUNTIME_POINTEE_PAIR_COPY_EXIT: &str = "calls/runtime_pointee_pair_copy_exit";
pub const RUNTIME_SLICE_ELEMENT_RUNTIME_INDEX_READ_EXIT: &str =
    "slices/runtime_slice_element_runtime_index_read_exit";
pub const RUNTIME_FIXED_VEC_ROUND_TRIP_EXIT: &str = "collections/runtime_fixed_vec_round_trip_exit";
pub const RUNTIME_ALIAS_INDEXED_READ_THROUGH_TRANSITION_EXIT: &str =
    "calls/runtime_alias_indexed_read_through_transition_exit";
pub const RUNTIME_FRAME_INDEXED_LOCAL_READ_EXIT: &str =
    "collections/runtime_frame_indexed_local_read_exit";
pub const RUNTIME_MACHINE_INDEXED_STRUCT_FIELD_ARG_EXIT: &str =
    "calls/runtime_machine_indexed_struct_field_arg_exit";
pub const RUNTIME_MACHINE_FRAME_INDEX_WRITE_EXIT: &str =
    "collections/runtime_machine_frame_index_write_exit";
pub const RUNTIME_FRAME_DOUBLE_INDEXED_READ_EXIT: &str =
    "collections/runtime_frame_double_indexed_read_exit";
pub const RUNTIME_DOUBLE_INDEXED_READ_EXIT: &str = "collections/runtime_double_indexed_read_exit";
pub const RUNTIME_DOUBLE_INDEXED_WRITE_EXIT: &str = "collections/runtime_double_indexed_write_exit";
pub const RUNTIME_DUAL_INDEXED_COPY_EXIT: &str = "collections/runtime_dual_indexed_copy_exit";
pub const RUNTIME_FRAME_MIXED_INDEX_PAIR_COPY_EXIT: &str =
    "collections/runtime_frame_mixed_index_pair_copy_exit";
pub const RUNTIME_CROSS_REGION_INDEXED_PAIR_COPY_EXIT: &str =
    "collections/runtime_cross_region_indexed_pair_copy_exit";
pub const RUNTIME_CROSS_REGION_DOUBLE_INDEXED_PAIR_COPY_EXIT: &str =
    "collections/runtime_cross_region_double_indexed_pair_copy_exit";
pub const F32_DEEP_CHAIN_BINARY: &str = "expressions/f32_deep_chain_binary";
pub const RUNTIME_STATEMENT_CALL_SINGLE_EXECUTION_EXIT: &str =
    "control_flow/runtime_statement_call_single_execution_exit";
pub const RUNTIME_SLICE_INDEXED_BINARY_RMW_EXIT: &str =
    "storage/runtime_slice_indexed_binary_rmw_exit";
pub const RUNTIME_DISPATCH_LOCAL_INDEX_BINARY_WRITE_EXIT: &str =
    "storage/runtime_dispatch_local_index_binary_write_exit";
pub const RUNTIME_INDEXED_RMW_LOOP_EXIT: &str = "collections/runtime_indexed_rmw_loop_exit";
pub const RUNTIME_DOUBLE_INDEXED_RMW_EXIT: &str = "collections/runtime_double_indexed_rmw_exit";
pub const RUNTIME_BOUNDED_CARRIER_LOCAL_SOURCE_CONCAT_EXIT: &str =
    "text/runtime_bounded_carrier_local_source_concat_exit";
pub const RUNTIME_STRING_APPEND_IN_PLACE_EXIT: &str = "text/runtime_string_append_in_place_exit";
pub const RUNTIME_LOCAL_ARRAY_INDEXED_STRING_FIELD_CONCAT_EXIT: &str =
    "text/runtime_local_array_indexed_string_field_concat_exit";
pub const RUNTIME_SLICE_ALIAS_INDEXED_STRING_FIELD_CONCAT_EXIT: &str =
    "text/runtime_slice_alias_indexed_string_field_concat_exit";
pub const RUNTIME_STRING_STORED_SUFFIX_EXIT: &str = "text/runtime_string_stored_suffix_exit";
pub const RUNTIME_RECORD_VIEW_EXIT: &str = "recast/runtime_record_view_exit";
pub const RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_BOUNDED_CARRIER_LITERAL_EXIT: &str =
    "text/runtime_machine_owned_double_indexed_bounded_carrier_literal_exit";
pub const RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_STRING_FIELD_CONCAT_EXIT: &str =
    "text/runtime_machine_owned_double_indexed_string_field_concat_exit";
pub const RUNTIME_X86_GENERAL_DOUBLE_INDEXED_STRING_CONCAT_COMPILE: &str =
    "text/runtime_x86_general_double_indexed_string_concat_compile";
pub const RUNTIME_WIRE_ENCODE_PRIMITIVE_EXIT: &str = "wire/runtime_wire_encode_primitive_exit";
pub const RUNTIME_WIRE_ENCODE_STRING_EXIT: &str = "wire/runtime_wire_encode_string_exit";
pub const RUNTIME_WIRE_ENCODE_BORROWED_SCALAR_SLICE_EXIT: &str =
    "wire/runtime_wire_encode_borrowed_scalar_slice_exit";
pub const RUNTIME_WIRE_ENCODE_REPEATED_THEN_STRING_EXIT: &str =
    "wire/runtime_wire_encode_repeated_then_string_exit";
pub const RUNTIME_WIRE_DECODE_BYTE_SLICE_EXIT: &str = "wire/runtime_wire_decode_byte_slice_exit";
pub const RUNTIME_WIRE_ROUNDTRIP_NESTED_AND_REPEATED_EXIT: &str =
    "wire/runtime_wire_roundtrip_nested_and_repeated_exit";
pub const RUNTIME_WIRE_DECODE_LET_COMPARE_EXIT: &str = "wire/runtime_wire_decode_let_compare_exit";
pub const RUNTIME_WIRE_DECODE_RANGED_FIELD_EXIT: &str =
    "wire/runtime_wire_decode_ranged_field_exit";
pub const RUNTIME_AARCH64_CROSS_REGION_FRAME_INDEXED_RMW_COMPILE: &str =
    "slices/runtime_aarch64_cross_region_frame_indexed_rmw_compile";
pub const RUNTIME_PLAN_LAID_COMPACT_BITS_EXIT: &str = "layouts/runtime_plan_laid_compact_bits_exit";
pub const RUNTIME_ENTRY_CAST_RESULT_EXIT: &str = "control_flow/runtime_entry_cast_result_exit";
pub const RUNTIME_NUMBER_TO_DECIMAL_EXIT: &str = "text/runtime_number_to_decimal_exit";
pub const RUNTIME_VIEW_OF_VIEW_CHAIN_EXIT: &str = "borrow/runtime_view_of_view_chain_exit";
pub const RUNTIME_MACHINE_OWNED_INDEXED_INTEGER_WRITE_EXIT: &str =
    "storage/runtime_machine_owned_indexed_integer_write_exit";
pub const RUNTIME_STRING_FIELD_LITERAL_GUARD_EXIT: &str =
    "text/runtime_string_field_literal_guard_exit";

pub const PASS_CANARIES: &[&str] = &[
    RUNTIME_INTEGER_LITERAL_DISPATCH_EXIT,
    RUNTIME_LOCAL_STRUCT_STRING_FIELD_CONCAT_EXIT,
    TERMINATION_INDEX_DISTANCE_COMPILE,
    RUNTIME_VALUE_CALL_THROUGH_ALIAS_IN_DISPATCH_EXIT,
    RUNTIME_SHARED_REF_PARAM_COPY_EXIT,
    RUNTIME_POINTEE_PAIR_COPY_EXIT,
    RUNTIME_SLICE_ELEMENT_RUNTIME_INDEX_READ_EXIT,
    RUNTIME_FIXED_VEC_ROUND_TRIP_EXIT,
    RUNTIME_ALIAS_INDEXED_READ_THROUGH_TRANSITION_EXIT,
    RUNTIME_FRAME_INDEXED_LOCAL_READ_EXIT,
    RUNTIME_MACHINE_INDEXED_STRUCT_FIELD_ARG_EXIT,
    RUNTIME_MACHINE_FRAME_INDEX_WRITE_EXIT,
    RUNTIME_FRAME_DOUBLE_INDEXED_READ_EXIT,
    RUNTIME_DOUBLE_INDEXED_READ_EXIT,
    RUNTIME_DOUBLE_INDEXED_WRITE_EXIT,
    RUNTIME_DUAL_INDEXED_COPY_EXIT,
    RUNTIME_FRAME_MIXED_INDEX_PAIR_COPY_EXIT,
    RUNTIME_CROSS_REGION_INDEXED_PAIR_COPY_EXIT,
    RUNTIME_CROSS_REGION_DOUBLE_INDEXED_PAIR_COPY_EXIT,
    F32_DEEP_CHAIN_BINARY,
    RUNTIME_STATEMENT_CALL_SINGLE_EXECUTION_EXIT,
    RUNTIME_SLICE_INDEXED_BINARY_RMW_EXIT,
    RUNTIME_DISPATCH_LOCAL_INDEX_BINARY_WRITE_EXIT,
    RUNTIME_INDEXED_RMW_LOOP_EXIT,
    RUNTIME_DOUBLE_INDEXED_RMW_EXIT,
    RUNTIME_BOUNDED_CARRIER_LOCAL_SOURCE_CONCAT_EXIT,
    RUNTIME_STRING_APPEND_IN_PLACE_EXIT,
    RUNTIME_LOCAL_ARRAY_INDEXED_STRING_FIELD_CONCAT_EXIT,
    RUNTIME_SLICE_ALIAS_INDEXED_STRING_FIELD_CONCAT_EXIT,
    RUNTIME_STRING_STORED_SUFFIX_EXIT,
    RUNTIME_RECORD_VIEW_EXIT,
    RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_BOUNDED_CARRIER_LITERAL_EXIT,
    RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_STRING_FIELD_CONCAT_EXIT,
    RUNTIME_X86_GENERAL_DOUBLE_INDEXED_STRING_CONCAT_COMPILE,
    RUNTIME_WIRE_ENCODE_PRIMITIVE_EXIT,
    RUNTIME_WIRE_ENCODE_STRING_EXIT,
    RUNTIME_WIRE_ENCODE_BORROWED_SCALAR_SLICE_EXIT,
    RUNTIME_WIRE_ENCODE_REPEATED_THEN_STRING_EXIT,
    RUNTIME_WIRE_DECODE_BYTE_SLICE_EXIT,
    RUNTIME_WIRE_ROUNDTRIP_NESTED_AND_REPEATED_EXIT,
    RUNTIME_WIRE_DECODE_LET_COMPARE_EXIT,
    RUNTIME_WIRE_DECODE_RANGED_FIELD_EXIT,
    RUNTIME_AARCH64_CROSS_REGION_FRAME_INDEXED_RMW_COMPILE,
    RUNTIME_PLAN_LAID_COMPACT_BITS_EXIT,
    RUNTIME_ENTRY_CAST_RESULT_EXIT,
    RUNTIME_NUMBER_TO_DECIMAL_EXIT,
    RUNTIME_VIEW_OF_VIEW_CHAIN_EXIT,
    RUNTIME_MACHINE_OWNED_INDEXED_INTEGER_WRITE_EXIT,
    RUNTIME_STRING_FIELD_LITERAL_GUARD_EXIT,
];
