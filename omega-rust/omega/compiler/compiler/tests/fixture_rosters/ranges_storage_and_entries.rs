//! Corpus inputs used by the range, storage, and entry-result tests.
//! Execution stages, native assertions, and scratch names stay with their owners.

pub(crate) const RUNTIME_GUARDED_RUNTIME_INDEX_INCREMENT_EXIT: &str =
    "range/runtime_guarded_runtime_index_increment_exit";
pub(crate) const RUNTIME_GUARDED_ELEMENT_INCREMENT_EXIT: &str =
    "range/runtime_guarded_element_increment_exit";
pub(crate) const RUNTIME_ELEMENT_RANGE_DATAFLOW_EXIT: &str =
    "range/runtime_element_range_dataflow_exit";
pub(crate) const RUNTIME_FUNNEL_GUARD_AGREEMENT_EXIT: &str =
    "range/runtime_funnel_guard_agreement_exit";
pub(crate) const RUNTIME_GUARDED_BINARY_OPERAND_EXIT: &str =
    "range/runtime_guarded_binary_operand_exit";
pub(crate) const RUNTIME_GUARDED_COPY_NARROWING_EXIT: &str =
    "range/runtime_guarded_copy_narrowing_exit";
pub(crate) const RUNTIME_RANGED_DIVIDE_MODULO_CHAIN_EXIT: &str =
    "arithmetic/runtime_ranged_divide_modulo_chain_exit";
pub(crate) const RUNTIME_RANGED_BITWISE_AND_MASK_EXIT: &str =
    "arithmetic/runtime_ranged_bitwise_and_mask_exit";
pub(crate) const RUNTIME_DECLARED_RANGE_INDEX_READ_EXIT: &str =
    "collections/runtime_declared_range_index_read_exit";
pub(crate) const RUNTIME_DECLARED_RANGE_INDEX_WRITE_EXIT: &str =
    "collections/runtime_declared_range_index_write_exit";
pub(crate) const RUNTIME_EXPRESSION_RANGE_BOUND_EXIT: &str =
    "arithmetic/runtime_expression_range_bound_exit";
pub(crate) const RUNTIME_INDEXED_STRUCT_FIELD_RMW_EXIT: &str =
    "collections/runtime_indexed_struct_field_rmw_exit";
pub(crate) const RUNTIME_INDEXED_STRUCT_FIELD_OPERAND_EXIT: &str =
    "collections/runtime_indexed_struct_field_operand_exit";
pub(crate) const RUNTIME_MACHINE_INDEXED_ARG_EXIT: &str = "calls/runtime_machine_indexed_arg_exit";
pub(crate) const RUNTIME_MACHINE_INDEXED_STRUCT_FIELD_ARG_EXIT: &str =
    "calls/runtime_machine_indexed_struct_field_arg_exit";
pub(crate) const RUNTIME_FRAME_INDEXED_PARAM_READ_EXIT: &str =
    "collections/runtime_frame_indexed_param_read_exit";
pub(crate) const RUNTIME_FRAME_INDEXED_PARAM_OPERAND_ARG_EXIT: &str =
    "collections/runtime_frame_indexed_param_operand_arg_exit";
pub(crate) const RUNTIME_FRAME_INDEXED_PARAM_FIELD_EXIT: &str =
    "collections/runtime_frame_indexed_param_field_exit";
pub(crate) const RUNTIME_FRAME_INDEXED_LOCAL_READ_EXIT: &str =
    "collections/runtime_frame_indexed_local_read_exit";
pub(crate) const RUNTIME_FRAME_INDEXED_BYTE_PARAM_READ_EXIT: &str =
    "collections/runtime_frame_indexed_byte_param_read_exit";
pub(crate) const RUNTIME_VALUE_MACHINE_PARAM_ARRAY_INDEX_EXIT: &str =
    "calls/runtime_value_machine_param_array_index_exit";
pub(crate) const RUNTIME_MACHINE_FRAME_INDEX_READ_EXIT: &str =
    "collections/runtime_machine_frame_index_read_exit";
pub(crate) const RUNTIME_MACHINE_FRAME_INDEX_WRITE_EXIT: &str =
    "collections/runtime_machine_frame_index_write_exit";
pub(crate) const RUNTIME_MACHINE_FRAME_INDEX_DUAL_FRAME_WRITE_EXIT: &str =
    "collections/runtime_machine_frame_index_dual_frame_write_exit";
pub(crate) const RUNTIME_MACHINE_FRAME_INDEX_RMW_EXIT: &str =
    "collections/runtime_machine_frame_index_rmw_exit";
pub(crate) const RUNTIME_MACHINE_FRAME_INDEX_ARG_OPERAND_EXIT: &str =
    "calls/runtime_machine_frame_index_arg_operand_exit";
pub(crate) const RUNTIME_NESTED_CONST_ROW_INDEXED_READ_EXIT: &str =
    "collections/runtime_nested_const_row_indexed_read_exit";
pub(crate) const RUNTIME_NESTED_CONST_ROW_STRUCT_FIELD_WRITE_EXIT: &str =
    "collections/runtime_nested_const_row_struct_field_write_exit";
pub(crate) const RUNTIME_NESTED_MIDDLE_INDEX_3D_EXIT: &str =
    "collections/runtime_nested_middle_index_3d_exit";
pub(crate) const RUNTIME_LET_BOUND_COMPUTED_INDEX_EXIT: &str =
    "collections/runtime_let_bound_computed_index_exit";
pub(crate) const RUNTIME_STRUCT_FIELD_OPERAND_MATRIX_EXIT: &str =
    "collections/runtime_struct_field_operand_matrix_exit";
pub(crate) const RUNTIME_STRUCT_FIELD_OPERAND_PARAM_EXIT: &str =
    "collections/runtime_struct_field_operand_param_exit";
pub(crate) const RUNTIME_DOUBLE_INDEXED_READ_EXIT: &str =
    "collections/runtime_double_indexed_read_exit";
pub(crate) const RUNTIME_NESTED_DEEP_CONST_PREFIX_EXIT: &str =
    "collections/runtime_nested_deep_const_prefix_exit";
pub(crate) const RUNTIME_DUAL_FRAME_INDEX_COPY_EXIT: &str =
    "collections/runtime_dual_frame_index_copy_exit";
pub(crate) const RUNTIME_FRAME_MIXED_INDEX_PAIR_COPY_EXIT: &str =
    "collections/runtime_frame_mixed_index_pair_copy_exit";
pub(crate) const RUNTIME_CROSS_REGION_INDEXED_PAIR_COPY_EXIT: &str =
    "collections/runtime_cross_region_indexed_pair_copy_exit";
pub(crate) const RUNTIME_CROSS_REGION_DOUBLE_INDEXED_PAIR_COPY_EXIT: &str =
    "collections/runtime_cross_region_double_indexed_pair_copy_exit";
pub(crate) const CONSTANT_NESTED_INDEX_GUARD_EXIT: &str =
    "collections/constant_nested_index_guard_exit";
pub(crate) const RUNTIME_DUAL_MIXED_INDEX_COPY_EXIT: &str =
    "collections/runtime_dual_mixed_index_copy_exit";
pub(crate) const RUNTIME_DURATION_CONSTRUCTORS_EXIT: &str =
    "time/runtime_duration_constructors_exit";
pub(crate) const RUNTIME_SLICE_ELEMENT_MACHINE_ROUNDTRIP_EXIT: &str =
    "slices/runtime_slice_element_machine_roundtrip_exit";
pub(crate) const RUNTIME_SLICE_ELEMENT_RUNTIME_INDEX_READ_EXIT: &str =
    "slices/runtime_slice_element_runtime_index_read_exit";
pub(crate) const RUNTIME_MEMBER_ARG_NESTED_READ_EXIT: &str =
    "calls/runtime_member_arg_nested_read_exit";
pub(crate) const RUNTIME_CONSTRUCTOR_COMPUTED_FIELD_EXIT: &str =
    "calls/runtime_constructor_computed_field_exit";
pub(crate) const RUNTIME_MACHINE_BOUNDED_SUBSLICE_LOCAL_EXIT: &str =
    "slices/runtime_machine_bounded_subslice_local_exit";
pub(crate) const RUNTIME_SUBSLICE_START_POINTER_EXIT: &str =
    "slices/runtime_subslice_start_pointer_exit";
pub(crate) const RUNTIME_LOOP_ACCUMULATOR_EXIT: &str = "calls/runtime_loop_accumulator_exit";
pub(crate) const RUNTIME_LOOP_ROTATION_EXIT: &str = "calls/runtime_loop_rotation_exit";
pub(crate) const RUNTIME_POST_CLAUSES_RETURN_TYPE_EXIT: &str =
    "calls/runtime_post_clauses_return_type_exit";
pub(crate) const RUNTIME_SLICE_LENGTH_LOCAL_BINDING_EXIT: &str =
    "calls/runtime_slice_length_local_binding_exit";
pub(crate) const RUNTIME_SLICE_LENGTH_LOCAL_PARAM_BINDING_EXIT: &str =
    "calls/runtime_slice_length_local_param_binding_exit";
pub(crate) const RUNTIME_SUBSLICE_LENGTH_LOCAL_BINDING_EXIT: &str =
    "calls/runtime_subslice_length_local_binding_exit";
pub(crate) const RUNTIME_INLINE_SUBSLICE_LENGTH_EXIT: &str =
    "calls/runtime_inline_subslice_length_exit";
pub(crate) const RUNTIME_END_FIXED_ARRAY_SUBSLICE_LOCAL_EXIT: &str =
    "slices/runtime_end_fixed_array_subslice_local_exit";
pub(crate) const RUNTIME_END_FIXED_ARRAY_SUBSLICE_ELEMENT_EXIT: &str =
    "slices/runtime_end_fixed_array_subslice_element_exit";
pub(crate) const GUARD_FIXED_ARRAY_LEN_OPERAND_EXIT: &str =
    "slices/guard_fixed_array_len_operand_exit";
pub(crate) const RUNTIME_BOUNDED_FIXED_ARRAY_SUBSLICE_ARG_EXIT: &str =
    "slices/runtime_bounded_fixed_array_subslice_arg_exit";
pub(crate) const RUNTIME_BOUNDED_CARRIER_CONCAT_EXIT: &str =
    "text/runtime_bounded_carrier_concat_exit";
pub(crate) const RUNTIME_BOUNDED_CARRIER_ALIAS_CONCAT_EXIT: &str =
    "text/runtime_bounded_carrier_alias_concat_exit";
pub(crate) const RUNTIME_BOUNDED_CARRIER_LOCAL_SOURCE_CONCAT_EXIT: &str =
    "text/runtime_bounded_carrier_local_source_concat_exit";
pub(crate) const RUNTIME_VALUE_CALL_SLICE_VIEW_CARRIER_GUARD_EXIT: &str =
    "text/runtime_value_call_slice_view_carrier_guard_exit";
pub(crate) const RUNTIME_VALUE_CALL_SLICE_VIEW_ELEMENT_ARG_EXIT: &str =
    "calls/runtime_value_call_slice_view_element_arg_exit";
pub(crate) const RUNTIME_LINEAR_SEARCH_EARLY_EXIT: &str =
    "control_flow/runtime_linear_search_early_exit";
pub(crate) const RUNTIME_ENTRY_RETURN_FIELD_EXIT: &str =
    "control_flow/runtime_entry_return_field_exit";
pub(crate) const RUNTIME_ENTRY_UNARY_RESULT_EXIT: &str =
    "control_flow/runtime_entry_unary_result_exit";
pub(crate) const RUNTIME_ENTRY_CAST_RESULT_EXIT: &str =
    "control_flow/runtime_entry_cast_result_exit";
pub(crate) const RUNTIME_ENTRY_NESTED_BINARY_RESULT_EXIT: &str =
    "control_flow/runtime_entry_nested_binary_result_exit";
pub(crate) const FREE_STANDING_MACHINE_HELPER_COMPILE: &str =
    "calls/free_standing_machine_helper_compile";
pub(crate) const RUNTIME_LOOP_PATTERNS_EXIT: &str = "control_flow/runtime_loop_patterns_exit";
pub(crate) const RUNTIME_COMPOSITE_INITIALIZER_LOCAL_ARG_EXIT: &str =
    "control_flow/runtime_composite_initializer_local_arg_exit";
pub(crate) const RUNTIME_CAPTURED_LOCAL_REMUTATED_FIELD_EXIT: &str =
    "control_flow/runtime_captured_local_remutated_field_exit";
pub(crate) const RUNTIME_BOUNDED_CARRIER_POINTEE_GUARD_EXIT: &str =
    "text/runtime_bounded_carrier_pointee_guard_exit";
pub(crate) const RUNTIME_BOUNDED_CARRIER_SLICE_FIELD_WRITE_EXIT: &str =
    "text/runtime_bounded_carrier_slice_field_write_exit";
pub(crate) const RUNTIME_BOUNDED_CARRIER_WRITE_LINE_EXIT: &str =
    "text/runtime_bounded_carrier_write_line_exit";
pub(crate) const RUNTIME_TEXT_BUILDER: &str = "text/runtime_text_builder";
pub(crate) const UTF8_RETURN_VIEW_EQUALS_EXIT: &str = "domains/utf8_return_view_equals_exit";
pub(crate) const RUNTIME_SHIFT_OPERATORS_EXIT: &str = "operators/runtime_shift_operators_exit";
pub(crate) const RUNTIME_BITWISE_OPERATORS_EXIT: &str = "operators/runtime_bitwise_operators_exit";
pub(crate) const RUNTIME_POPCOUNT_LOOP_EXIT: &str = "operators/runtime_popcount_loop_exit";
pub(crate) const RUNTIME_XORSHIFT_PRNG_EXIT: &str = "operators/runtime_xorshift_prng_exit";
pub(crate) const RUNTIME_BITWISE_GUARD_EXIT: &str = "operators/runtime_bitwise_guard_exit";
pub(crate) const INTEGER_LITERAL_SUFFIX_EXIT: &str = "operators/integer_literal_suffix_exit";
pub(crate) const RUNTIME_VALUE_POSITION_BRANCHING_CALL_EXIT: &str =
    "calls/runtime_value_position_branching_call_exit";
pub(crate) const RUNTIME_FREE_MACHINE_VALUE_CALL_EXIT: &str =
    "calls/runtime_free_machine_value_call_exit";
pub(crate) const RUNTIME_FREE_MACHINE_STRUCT_ARG_EXIT: &str =
    "calls/runtime_free_machine_struct_arg_exit";
pub(crate) const BY_VALUE_CASE_PARAM_SELF_WRITE_EXIT: &str =
    "calls/by_value_case_param_self_write_exit";
pub(crate) const RUNTIME_ATTACHED_MACHINE_STRUCT_ARG_EXIT: &str =
    "calls/runtime_attached_machine_struct_arg_exit";
pub(crate) const RUNTIME_RECORD_FORWARDING_STATEMENT_CALL_EXIT: &str =
    "calls/runtime_record_forwarding_statement_call_exit";
pub(crate) const RUNTIME_FREE_MACHINE_STRUCT_RETURN_EXIT: &str =
    "calls/runtime_free_machine_struct_return_exit";
pub(crate) const RUNTIME_FREE_MACHINE_VALUE_CALL_MUT_ARG_EXIT: &str =
    "calls/runtime_free_machine_value_call_mut_arg_exit";
pub(crate) const RUNTIME_FREE_MACHINE_LOOPING_VALUE_CALL_EXIT: &str =
    "calls/runtime_free_machine_looping_value_call_exit";
pub(crate) const RUNTIME_NUMERIC_CAST_EXIT: &str = "expressions/runtime_numeric_cast_exit";
pub(crate) const RUNTIME_WIDENED_COMPARISON_EXIT: &str =
    "expressions/runtime_widened_comparison_exit";
pub(crate) const RUNTIME_WIDENED_BITWISE_EXIT: &str = "expressions/runtime_widened_bitwise_exit";
pub(crate) const RUNTIME_16BIT_CAST_EXIT: &str = "expressions/runtime_16bit_cast_exit";
pub(crate) const RUNTIME_FLOAT_PLACE_COMPARISON_EXIT: &str =
    "expressions/runtime_float_place_comparison_exit";
pub(crate) const RUNTIME_FLOAT_COMPARISON_EXIT: &str = "expressions/runtime_float_comparison_exit";
pub(crate) const RUNTIME_FLOAT_ARITHMETIC_EXIT: &str = "expressions/runtime_float_arithmetic_exit";
pub(crate) const RUNTIME_VERSION_MIGRATION_EXIT: &str = "versioning/runtime_version_migration_exit";
pub(crate) const RUNTIME_VERSIONED_MATCH_ZII_EXIT: &str =
    "versioning/runtime_versioned_match_zii_exit";
pub(crate) const RUNTIME_VERSIONED_THREE_ERA_MATCH_ZII_EXIT: &str =
    "versioning/runtime_versioned_three_era_match_zii_exit";
pub(crate) const RUNTIME_EQUATABLE_SCALAR_NOT_EQUALS_GUARD_EXIT: &str =
    "traits/runtime_equatable_scalar_not_equals_guard_exit";
pub(crate) const RUNTIME_CASE_MEMBERSHIP_MIXED_SHAPE_EXIT: &str =
    "data/runtime_case_membership_mixed_shape_exit";
pub(crate) const RUNTIME_WIRE_ROUNDTRIP_REPEATED_MAX_ONE_EXIT: &str =
    "wire/runtime_wire_roundtrip_repeated_max_one_exit";
pub(crate) const RUNTIME_WIRE_ROUNDTRIP_UTF8_EXIT: &str = "wire/runtime_wire_roundtrip_utf8_exit";
pub(crate) const RUNTIME_WIRE_UTF8_EDGE_VERDICTS_EXIT: &str =
    "wire/runtime_wire_utf8_edge_verdicts_exit";
pub(crate) const RUNTIME_WIRE_UTF8_INVALID_REFUSED_EXIT: &str =
    "wire/runtime_wire_utf8_invalid_refused_exit";
pub(crate) const RUNTIME_WIRE_SCHEMA_AS_VALUE_TYPE_EXIT: &str =
    "wire/runtime_wire_schema_as_value_type_exit";
pub(crate) const RUNTIME_WIRE_DECODE_LET_COMPARE_EXIT: &str =
    "wire/runtime_wire_decode_let_compare_exit";
pub(crate) const RUNTIME_WIRE_ENCODE_REPEATED_THEN_STRING_EXIT: &str =
    "wire/runtime_wire_encode_repeated_then_string_exit";
pub(crate) const RUNTIME_WIRE_ROUNDTRIP_NESTED_AND_REPEATED_EXIT: &str =
    "wire/runtime_wire_roundtrip_nested_and_repeated_exit";
pub(crate) const RUNTIME_CONST_ARRAY_LENGTH_TRANSITIVE_EXIT: &str =
    "comptime/runtime_const_array_length_transitive_exit";
pub(crate) const RUNTIME_CONST_ARRAY_LENGTH_BARE_CALL_ARM_EXIT: &str =
    "comptime/runtime_const_array_length_bare_call_arm_exit";

pub(crate) struct EntryScalarOperationResult {
    pub(crate) name: &'static str,
    pub(crate) path: &'static str,
    pub(crate) expected: i32,
}

// One authored basename supplies both the existing scratch tag and corpus path.
macro_rules! entry_scalar_operation_results {
    ($(($name:literal, $expected:literal)),+ $(,)?) => {
        pub(crate) const ENTRY_SCALAR_OPERATION_RESULTS: &[EntryScalarOperationResult] = &[
            $(EntryScalarOperationResult {
                name: $name,
                path: concat!("control_flow/", $name),
                expected: $expected,
            }),+
        ];
    };
}

entry_scalar_operation_results! {
    ("runtime_entry_builtin_result_exit", 70),
    ("runtime_entry_comparison_result_exit", 1),
}

pub(crate) const PASS_CANARIES: &[&str] = &[
    RUNTIME_GUARDED_RUNTIME_INDEX_INCREMENT_EXIT,
    RUNTIME_GUARDED_ELEMENT_INCREMENT_EXIT,
    RUNTIME_ELEMENT_RANGE_DATAFLOW_EXIT,
    RUNTIME_FUNNEL_GUARD_AGREEMENT_EXIT,
    RUNTIME_GUARDED_BINARY_OPERAND_EXIT,
    RUNTIME_GUARDED_COPY_NARROWING_EXIT,
    RUNTIME_RANGED_DIVIDE_MODULO_CHAIN_EXIT,
    RUNTIME_RANGED_BITWISE_AND_MASK_EXIT,
    RUNTIME_DECLARED_RANGE_INDEX_READ_EXIT,
    RUNTIME_DECLARED_RANGE_INDEX_WRITE_EXIT,
    RUNTIME_EXPRESSION_RANGE_BOUND_EXIT,
    RUNTIME_INDEXED_STRUCT_FIELD_RMW_EXIT,
    RUNTIME_INDEXED_STRUCT_FIELD_OPERAND_EXIT,
    RUNTIME_MACHINE_INDEXED_ARG_EXIT,
    RUNTIME_MACHINE_INDEXED_STRUCT_FIELD_ARG_EXIT,
    RUNTIME_FRAME_INDEXED_PARAM_READ_EXIT,
    RUNTIME_FRAME_INDEXED_PARAM_OPERAND_ARG_EXIT,
    RUNTIME_FRAME_INDEXED_PARAM_FIELD_EXIT,
    RUNTIME_FRAME_INDEXED_LOCAL_READ_EXIT,
    RUNTIME_FRAME_INDEXED_BYTE_PARAM_READ_EXIT,
    RUNTIME_VALUE_MACHINE_PARAM_ARRAY_INDEX_EXIT,
    RUNTIME_MACHINE_FRAME_INDEX_READ_EXIT,
    RUNTIME_MACHINE_FRAME_INDEX_WRITE_EXIT,
    RUNTIME_MACHINE_FRAME_INDEX_DUAL_FRAME_WRITE_EXIT,
    RUNTIME_MACHINE_FRAME_INDEX_RMW_EXIT,
    RUNTIME_MACHINE_FRAME_INDEX_ARG_OPERAND_EXIT,
    RUNTIME_NESTED_CONST_ROW_INDEXED_READ_EXIT,
    RUNTIME_NESTED_CONST_ROW_STRUCT_FIELD_WRITE_EXIT,
    RUNTIME_NESTED_MIDDLE_INDEX_3D_EXIT,
    RUNTIME_LET_BOUND_COMPUTED_INDEX_EXIT,
    RUNTIME_STRUCT_FIELD_OPERAND_MATRIX_EXIT,
    RUNTIME_STRUCT_FIELD_OPERAND_PARAM_EXIT,
    RUNTIME_DOUBLE_INDEXED_READ_EXIT,
    RUNTIME_NESTED_DEEP_CONST_PREFIX_EXIT,
    RUNTIME_DUAL_FRAME_INDEX_COPY_EXIT,
    RUNTIME_FRAME_MIXED_INDEX_PAIR_COPY_EXIT,
    RUNTIME_CROSS_REGION_INDEXED_PAIR_COPY_EXIT,
    RUNTIME_CROSS_REGION_DOUBLE_INDEXED_PAIR_COPY_EXIT,
    CONSTANT_NESTED_INDEX_GUARD_EXIT,
    RUNTIME_DUAL_MIXED_INDEX_COPY_EXIT,
    RUNTIME_DURATION_CONSTRUCTORS_EXIT,
    RUNTIME_SLICE_ELEMENT_MACHINE_ROUNDTRIP_EXIT,
    RUNTIME_SLICE_ELEMENT_RUNTIME_INDEX_READ_EXIT,
    RUNTIME_MEMBER_ARG_NESTED_READ_EXIT,
    RUNTIME_CONSTRUCTOR_COMPUTED_FIELD_EXIT,
    RUNTIME_MACHINE_BOUNDED_SUBSLICE_LOCAL_EXIT,
    RUNTIME_SUBSLICE_START_POINTER_EXIT,
    RUNTIME_LOOP_ACCUMULATOR_EXIT,
    RUNTIME_LOOP_ROTATION_EXIT,
    RUNTIME_POST_CLAUSES_RETURN_TYPE_EXIT,
    RUNTIME_SLICE_LENGTH_LOCAL_BINDING_EXIT,
    RUNTIME_SLICE_LENGTH_LOCAL_PARAM_BINDING_EXIT,
    RUNTIME_SUBSLICE_LENGTH_LOCAL_BINDING_EXIT,
    RUNTIME_INLINE_SUBSLICE_LENGTH_EXIT,
    RUNTIME_END_FIXED_ARRAY_SUBSLICE_LOCAL_EXIT,
    RUNTIME_END_FIXED_ARRAY_SUBSLICE_ELEMENT_EXIT,
    GUARD_FIXED_ARRAY_LEN_OPERAND_EXIT,
    RUNTIME_BOUNDED_FIXED_ARRAY_SUBSLICE_ARG_EXIT,
    RUNTIME_BOUNDED_CARRIER_CONCAT_EXIT,
    RUNTIME_BOUNDED_CARRIER_ALIAS_CONCAT_EXIT,
    RUNTIME_BOUNDED_CARRIER_LOCAL_SOURCE_CONCAT_EXIT,
    RUNTIME_VALUE_CALL_SLICE_VIEW_CARRIER_GUARD_EXIT,
    RUNTIME_VALUE_CALL_SLICE_VIEW_ELEMENT_ARG_EXIT,
    RUNTIME_LINEAR_SEARCH_EARLY_EXIT,
    RUNTIME_ENTRY_RETURN_FIELD_EXIT,
    RUNTIME_ENTRY_UNARY_RESULT_EXIT,
    RUNTIME_ENTRY_CAST_RESULT_EXIT,
    RUNTIME_ENTRY_NESTED_BINARY_RESULT_EXIT,
    FREE_STANDING_MACHINE_HELPER_COMPILE,
    RUNTIME_LOOP_PATTERNS_EXIT,
    RUNTIME_COMPOSITE_INITIALIZER_LOCAL_ARG_EXIT,
    RUNTIME_CAPTURED_LOCAL_REMUTATED_FIELD_EXIT,
    RUNTIME_BOUNDED_CARRIER_POINTEE_GUARD_EXIT,
    RUNTIME_BOUNDED_CARRIER_SLICE_FIELD_WRITE_EXIT,
    RUNTIME_BOUNDED_CARRIER_WRITE_LINE_EXIT,
    RUNTIME_TEXT_BUILDER,
    UTF8_RETURN_VIEW_EQUALS_EXIT,
    RUNTIME_SHIFT_OPERATORS_EXIT,
    RUNTIME_BITWISE_OPERATORS_EXIT,
    RUNTIME_POPCOUNT_LOOP_EXIT,
    RUNTIME_XORSHIFT_PRNG_EXIT,
    RUNTIME_BITWISE_GUARD_EXIT,
    INTEGER_LITERAL_SUFFIX_EXIT,
    RUNTIME_VALUE_POSITION_BRANCHING_CALL_EXIT,
    RUNTIME_FREE_MACHINE_VALUE_CALL_EXIT,
    RUNTIME_FREE_MACHINE_STRUCT_ARG_EXIT,
    BY_VALUE_CASE_PARAM_SELF_WRITE_EXIT,
    RUNTIME_ATTACHED_MACHINE_STRUCT_ARG_EXIT,
    RUNTIME_RECORD_FORWARDING_STATEMENT_CALL_EXIT,
    RUNTIME_FREE_MACHINE_STRUCT_RETURN_EXIT,
    RUNTIME_FREE_MACHINE_VALUE_CALL_MUT_ARG_EXIT,
    RUNTIME_FREE_MACHINE_LOOPING_VALUE_CALL_EXIT,
    RUNTIME_NUMERIC_CAST_EXIT,
    RUNTIME_WIDENED_COMPARISON_EXIT,
    RUNTIME_WIDENED_BITWISE_EXIT,
    RUNTIME_16BIT_CAST_EXIT,
    RUNTIME_FLOAT_PLACE_COMPARISON_EXIT,
    RUNTIME_FLOAT_COMPARISON_EXIT,
    RUNTIME_FLOAT_ARITHMETIC_EXIT,
    RUNTIME_VERSION_MIGRATION_EXIT,
    RUNTIME_VERSIONED_MATCH_ZII_EXIT,
    RUNTIME_VERSIONED_THREE_ERA_MATCH_ZII_EXIT,
    RUNTIME_EQUATABLE_SCALAR_NOT_EQUALS_GUARD_EXIT,
    RUNTIME_CASE_MEMBERSHIP_MIXED_SHAPE_EXIT,
    RUNTIME_WIRE_ROUNDTRIP_REPEATED_MAX_ONE_EXIT,
    RUNTIME_WIRE_ROUNDTRIP_UTF8_EXIT,
    RUNTIME_WIRE_UTF8_EDGE_VERDICTS_EXIT,
    RUNTIME_WIRE_UTF8_INVALID_REFUSED_EXIT,
    RUNTIME_WIRE_SCHEMA_AS_VALUE_TYPE_EXIT,
    RUNTIME_WIRE_DECODE_LET_COMPARE_EXIT,
    RUNTIME_WIRE_ENCODE_REPEATED_THEN_STRING_EXIT,
    RUNTIME_WIRE_ROUNDTRIP_NESTED_AND_REPEATED_EXIT,
    RUNTIME_CONST_ARRAY_LENGTH_TRANSITIVE_EXIT,
    RUNTIME_CONST_ARRAY_LENGTH_BARE_CALL_ARM_EXIT,
];
