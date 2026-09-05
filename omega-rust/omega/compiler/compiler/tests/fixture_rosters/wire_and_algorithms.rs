//! Exact corpus inputs used by the wire and algorithm tests.
//! Native execution, checked compilation, reports, and inline diagnostics stay in the tests.

pub(crate) const RUNTIME_METHOD_VIEW_WRITE_AFTER_LAST_USE_EXIT: &str =
    "borrow/runtime_method_view_write_after_last_use_exit";
pub(crate) const RUNTIME_VIEW_OF_VIEW_CHAIN_EXIT: &str = "borrow/runtime_view_of_view_chain_exit";
pub(crate) const TERMINATION_SLICE_LENGTH_ORDER_UNIMPLEMENTED: &str =
    "slices/termination_slice_length_order_unimplemented";
pub(crate) const RUNTIME_SHRINKING_SLICE_RECURSION_EXIT: &str =
    "termination/runtime_shrinking_slice_recursion_exit";
pub(crate) const RUNTIME_WIRE_ENCODE_PRIMITIVE_EXIT: &str =
    "wire/runtime_wire_encode_primitive_exit";
pub(crate) const RUNTIME_WIRE_ENCODE_ERA_DISCRIMINATOR_EXIT: &str =
    "wire/runtime_wire_encode_era_discriminator_exit";
pub(crate) const NUMBERED_CASE_IDENTITIES: &str = "wire/numbered_case_identities";
pub(crate) const RUNTIME_WIRE_ROUNDTRIP_PRIMITIVE_EXIT: &str =
    "wire/runtime_wire_roundtrip_primitive_exit";
pub(crate) const RUNTIME_WIRE_DECODE_RANGED_FIELD_EXIT: &str =
    "wire/runtime_wire_decode_ranged_field_exit";
pub(crate) const RUNTIME_WIRE_DECODE_RANGED_REPEATED_EXIT: &str =
    "wire/runtime_wire_decode_ranged_repeated_exit";
pub(crate) const RUNTIME_WIRE_DECODE_REJECTS_NONCANONICAL_BOOL_EXIT: &str =
    "wire/runtime_wire_decode_rejects_noncanonical_bool_exit";
pub(crate) const RUNTIME_WIRE_DECODE_REJECTS_NONCANONICAL_VARINT_EXIT: &str =
    "wire/runtime_wire_decode_rejects_noncanonical_varint_exit";
pub(crate) const RUNTIME_WIRE_DECODE_REJECTS_SCALAR_WIDTH_OVERFLOW_EXIT: &str =
    "wire/runtime_wire_decode_rejects_scalar_width_overflow_exit";
pub(crate) const RUNTIME_WIRE_ROUNDTRIP_NESTED_EXIT: &str =
    "wire/runtime_wire_roundtrip_nested_exit";
pub(crate) const RUNTIME_WIRE_DECODE_REJECTS_BAD_NESTED_LENGTH_EXIT: &str =
    "wire/runtime_wire_decode_rejects_bad_nested_length_exit";
pub(crate) const RUNTIME_WIRE_ROUNDTRIP_REPEATED_EXIT: &str =
    "wire/runtime_wire_roundtrip_repeated_exit";
pub(crate) const RUNTIME_WIRE_DECODE_REJECTS_REPEATED_OVERFLOW_EXIT: &str =
    "wire/runtime_wire_decode_rejects_repeated_overflow_exit";
pub(crate) const RUNTIME_WIRE_DECODE_REJECTS_WRONG_ERA_EXIT: &str =
    "wire/runtime_wire_decode_rejects_wrong_era_exit";
pub(crate) const RUNTIME_WIRE_ENCODE_STRING_EXIT: &str = "wire/runtime_wire_encode_string_exit";
pub(crate) const RUNTIME_WIRE_ENCODE_BYTE_SLICE_EXIT: &str =
    "wire/runtime_wire_encode_byte_slice_exit";
pub(crate) const RUNTIME_WIRE_ENCODE_BORROWED_SCALAR_SLICE_EXIT: &str =
    "wire/runtime_wire_encode_borrowed_scalar_slice_exit";
pub(crate) const RUNTIME_WIRE_DECODE_BYTE_SLICE_EXIT: &str =
    "wire/runtime_wire_decode_byte_slice_exit";
pub(crate) const RUNTIME_WIRE_DECODED_BYTE_SLICE_INDEX_EXIT: &str =
    "wire/runtime_wire_decoded_byte_slice_index_exit";
pub(crate) const RUNTIME_WIRE_DECODED_BYTE_SLICE_LEN_EXIT: &str =
    "wire/runtime_wire_decoded_byte_slice_len_exit";
pub(crate) const RUNTIME_CALL_RESULT_BINARY_OPERAND_EXIT: &str =
    "expressions/runtime_call_result_binary_operand_exit";
pub(crate) const RUNTIME_CAST_OPERAND_EXIT: &str = "expressions/runtime_cast_operand_exit";
pub(crate) const RUNTIME_F32_ARITHMETIC_EXIT: &str = "expressions/runtime_f32_arithmetic_exit";
pub(crate) const RUNTIME_F32_LOCAL_ARITHMETIC_EXIT: &str =
    "expressions/runtime_f32_local_arithmetic_exit";
pub(crate) const RUNTIME_MULTI_ARM_VALUE_TRANSITION_EXIT: &str =
    "calls/runtime_multi_arm_value_transition_exit";
pub(crate) const RUNTIME_VALUE_TRANSITION_UNSIGNED_GUARD_EXIT: &str =
    "calls/runtime_value_transition_unsigned_guard_exit";
pub(crate) const RUNTIME_CONST_ARRAY_LENGTH_EXIT: &str = "comptime/runtime_const_array_length_exit";
pub(crate) const RUNTIME_FIXED_VEC_ROUND_TRIP_EXIT: &str =
    "collections/runtime_fixed_vec_round_trip_exit";
pub(crate) const RUNTIME_FLOAT_NEGATIVE_OPS_EXIT: &str =
    "arithmetic/runtime_float_negative_ops_exit";
pub(crate) const RUNTIME_FLOAT32_ARRAY_CONVERSION_EXIT: &str =
    "arithmetic/runtime_float32_array_conversion_exit";
pub(crate) const RUNTIME_VALUE_CALL_LET_COMBINE_EXIT: &str =
    "calls/runtime_value_call_let_combine_exit";
pub(crate) const RUNTIME_FLOAT_NAN_COMPARISON_EXIT: &str =
    "arithmetic/runtime_float_nan_comparison_exit";
pub(crate) const RUNTIME_SATURATING_DOMAIN_EXIT: &str = "arithmetic/runtime_saturating_domain_exit";
pub(crate) const RUNTIME_I64_SIGNED_ARITHMETIC_EXIT: &str =
    "arithmetic/runtime_i64_signed_arithmetic_exit";
pub(crate) const RUNTIME_CAST_SIGN_ZERO_EXTENSION_EXIT: &str =
    "arithmetic/runtime_cast_sign_zero_extension_exit";
pub(crate) const RUNTIME_BITWISE_HIGH_OPS_EXIT: &str = "arithmetic/runtime_bitwise_high_ops_exit";
pub(crate) const RUNTIME_UNSIGNED_HIGH_COMPARISON_EXIT: &str =
    "arithmetic/runtime_unsigned_high_comparison_exit";
pub(crate) const RUNTIME_SIGNED_MODULO_SHIFT_EDGES_EXIT: &str =
    "arithmetic/runtime_signed_modulo_shift_edges_exit";
pub(crate) const RUNTIME_NEWTON_SQRT_EXIT: &str = "arithmetic/runtime_newton_sqrt_exit";
pub(crate) const RUNTIME_MONTE_CARLO_PI_EXIT: &str = "arithmetic/runtime_monte_carlo_pi_exit";
pub(crate) const RUNTIME_GCD_EUCLID_EXIT: &str = "arithmetic/runtime_gcd_euclid_exit";
pub(crate) const RUNTIME_RPN_EVALUATOR_EXIT: &str = "collections/runtime_rpn_evaluator_exit";
pub(crate) const RUNTIME_ACTIVITY_SELECTION_GREEDY_EXIT: &str =
    "collections/runtime_activity_selection_greedy_exit";
pub(crate) const RUNTIME_MAZE_PATHFIND_EXIT: &str = "collections/runtime_maze_pathfind_exit";
pub(crate) const RUNTIME_NQUEENS_BACKTRACKING_EXIT: &str =
    "collections/runtime_nqueens_backtracking_exit";
pub(crate) const RUNTIME_COIN_CHANGE_DP_EXIT: &str = "collections/runtime_coin_change_dp_exit";
pub(crate) const RUNTIME_BFS_TRAVERSAL_EXIT: &str = "collections/runtime_bfs_traversal_exit";
pub(crate) const RUNTIME_HASH_TABLE_EXIT: &str = "collections/runtime_hash_table_exit";
pub(crate) const RUNTIME_MATRIX_MULTIPLY_EXIT: &str = "collections/runtime_matrix_multiply_exit";
pub(crate) const RUNTIME_RING_BUFFER_QUEUE_EXIT: &str =
    "collections/runtime_ring_buffer_queue_exit";
pub(crate) const RUNTIME_BUBBLE_SORT_EXIT: &str = "collections/runtime_bubble_sort_exit";
pub(crate) const RUNTIME_2D_TRANSPOSE_EXIT: &str = "collections/runtime_2d_transpose_exit";
pub(crate) const RUNTIME_INDEXED_THROUGH_GUARD_CHAIN_EXIT: &str =
    "collections/runtime_indexed_through_guard_chain_exit";
pub(crate) const RUNTIME_BINARY_SEARCH_EXIT: &str = "collections/runtime_binary_search_exit";
pub(crate) const RUNTIME_TWO_POINTER_PALINDROME_EXIT: &str =
    "collections/runtime_two_pointer_palindrome_exit";
pub(crate) const RUNTIME_NESTED_STRUCT_ARRAY_FIELD_EXIT: &str =
    "collections/runtime_nested_struct_array_field_exit";
pub(crate) const RUNTIME_ENUM_GRID_SCAN_EXIT: &str = "collections/runtime_enum_grid_scan_exit";
pub(crate) const RUNTIME_TWO_INDEXED_READS_BINARY_EXIT: &str =
    "collections/runtime_two_indexed_reads_binary_exit";
pub(crate) const RUNTIME_STRUCT_FIELD_TEMP_ARITH_EXIT: &str =
    "collections/runtime_struct_field_temp_arith_exit";
pub(crate) const RUNTIME_INDEXED_STRUCT_WRITE_LOOP_EXIT: &str =
    "collections/runtime_indexed_struct_write_loop_exit";
pub(crate) const STD_OPTION_RUNTIME_MATCH_EXIT: &str = "collections/std_option_runtime_match_exit";
pub(crate) const RUNTIME_INDEXED_READ_THEN_GUARD_EXIT: &str =
    "collections/runtime_indexed_read_then_guard_exit";
pub(crate) const RUNTIME_ROW_CONST_COLUMN_WRITE_EXIT: &str =
    "collections/runtime_row_const_column_write_exit";
pub(crate) const RUNTIME_NESTED_ARRAY_CONST_INDEX_EXIT: &str =
    "collections/runtime_nested_array_const_index_exit";
pub(crate) const RUNTIME_WHOLE_ARRAY_VALUE_COPY_EXIT: &str =
    "collections/runtime_whole_array_value_copy_exit";
pub(crate) const RUNTIME_WHOLE_STRUCT_VALUE_COPY_EXIT: &str =
    "collections/runtime_whole_struct_value_copy_exit";
pub(crate) const RUNTIME_RULE90_AUTOMATON_EXIT: &str = "collections/runtime_rule90_automaton_exit";
pub(crate) const RUNTIME_FIXED_ARRAY_FIELD_GUARD_EXIT: &str =
    "expressions/runtime_fixed_array_field_guard_exit";
pub(crate) const RUNTIME_FIXED_ARRAY_FIELD_VALUE_EXIT: &str =
    "expressions/runtime_fixed_array_field_value_exit";
pub(crate) const FIXED_ARRAY_ELEMENT_GUARD: &str = "control_flow/fixed_array_element_guard";

pub(crate) const PASS_CANARIES: &[&str] = &[
    RUNTIME_METHOD_VIEW_WRITE_AFTER_LAST_USE_EXIT,
    RUNTIME_VIEW_OF_VIEW_CHAIN_EXIT,
    RUNTIME_SHRINKING_SLICE_RECURSION_EXIT,
    RUNTIME_WIRE_ENCODE_PRIMITIVE_EXIT,
    RUNTIME_WIRE_ENCODE_ERA_DISCRIMINATOR_EXIT,
    NUMBERED_CASE_IDENTITIES,
    RUNTIME_WIRE_ROUNDTRIP_PRIMITIVE_EXIT,
    RUNTIME_WIRE_DECODE_RANGED_FIELD_EXIT,
    RUNTIME_WIRE_DECODE_RANGED_REPEATED_EXIT,
    RUNTIME_WIRE_DECODE_REJECTS_NONCANONICAL_BOOL_EXIT,
    RUNTIME_WIRE_DECODE_REJECTS_NONCANONICAL_VARINT_EXIT,
    RUNTIME_WIRE_DECODE_REJECTS_SCALAR_WIDTH_OVERFLOW_EXIT,
    RUNTIME_WIRE_ROUNDTRIP_NESTED_EXIT,
    RUNTIME_WIRE_DECODE_REJECTS_BAD_NESTED_LENGTH_EXIT,
    RUNTIME_WIRE_ROUNDTRIP_REPEATED_EXIT,
    RUNTIME_WIRE_DECODE_REJECTS_REPEATED_OVERFLOW_EXIT,
    RUNTIME_WIRE_DECODE_REJECTS_WRONG_ERA_EXIT,
    RUNTIME_WIRE_ENCODE_STRING_EXIT,
    RUNTIME_WIRE_ENCODE_BYTE_SLICE_EXIT,
    RUNTIME_WIRE_ENCODE_BORROWED_SCALAR_SLICE_EXIT,
    RUNTIME_WIRE_DECODE_BYTE_SLICE_EXIT,
    RUNTIME_WIRE_DECODED_BYTE_SLICE_INDEX_EXIT,
    RUNTIME_WIRE_DECODED_BYTE_SLICE_LEN_EXIT,
    RUNTIME_CALL_RESULT_BINARY_OPERAND_EXIT,
    RUNTIME_CAST_OPERAND_EXIT,
    RUNTIME_F32_ARITHMETIC_EXIT,
    RUNTIME_F32_LOCAL_ARITHMETIC_EXIT,
    RUNTIME_MULTI_ARM_VALUE_TRANSITION_EXIT,
    RUNTIME_VALUE_TRANSITION_UNSIGNED_GUARD_EXIT,
    RUNTIME_CONST_ARRAY_LENGTH_EXIT,
    RUNTIME_FIXED_VEC_ROUND_TRIP_EXIT,
    RUNTIME_FLOAT_NEGATIVE_OPS_EXIT,
    RUNTIME_FLOAT32_ARRAY_CONVERSION_EXIT,
    RUNTIME_VALUE_CALL_LET_COMBINE_EXIT,
    RUNTIME_FLOAT_NAN_COMPARISON_EXIT,
    RUNTIME_SATURATING_DOMAIN_EXIT,
    RUNTIME_I64_SIGNED_ARITHMETIC_EXIT,
    RUNTIME_CAST_SIGN_ZERO_EXTENSION_EXIT,
    RUNTIME_BITWISE_HIGH_OPS_EXIT,
    RUNTIME_UNSIGNED_HIGH_COMPARISON_EXIT,
    RUNTIME_SIGNED_MODULO_SHIFT_EDGES_EXIT,
    RUNTIME_NEWTON_SQRT_EXIT,
    RUNTIME_MONTE_CARLO_PI_EXIT,
    RUNTIME_GCD_EUCLID_EXIT,
    RUNTIME_RPN_EVALUATOR_EXIT,
    RUNTIME_ACTIVITY_SELECTION_GREEDY_EXIT,
    RUNTIME_MAZE_PATHFIND_EXIT,
    RUNTIME_NQUEENS_BACKTRACKING_EXIT,
    RUNTIME_COIN_CHANGE_DP_EXIT,
    RUNTIME_BFS_TRAVERSAL_EXIT,
    RUNTIME_HASH_TABLE_EXIT,
    RUNTIME_MATRIX_MULTIPLY_EXIT,
    RUNTIME_RING_BUFFER_QUEUE_EXIT,
    RUNTIME_BUBBLE_SORT_EXIT,
    RUNTIME_2D_TRANSPOSE_EXIT,
    RUNTIME_INDEXED_THROUGH_GUARD_CHAIN_EXIT,
    RUNTIME_BINARY_SEARCH_EXIT,
    RUNTIME_TWO_POINTER_PALINDROME_EXIT,
    RUNTIME_NESTED_STRUCT_ARRAY_FIELD_EXIT,
    RUNTIME_ENUM_GRID_SCAN_EXIT,
    RUNTIME_TWO_INDEXED_READS_BINARY_EXIT,
    RUNTIME_STRUCT_FIELD_TEMP_ARITH_EXIT,
    RUNTIME_INDEXED_STRUCT_WRITE_LOOP_EXIT,
    STD_OPTION_RUNTIME_MATCH_EXIT,
    RUNTIME_INDEXED_READ_THEN_GUARD_EXIT,
    RUNTIME_ROW_CONST_COLUMN_WRITE_EXIT,
    RUNTIME_NESTED_ARRAY_CONST_INDEX_EXIT,
    RUNTIME_WHOLE_ARRAY_VALUE_COPY_EXIT,
    RUNTIME_WHOLE_STRUCT_VALUE_COPY_EXIT,
    RUNTIME_RULE90_AUTOMATON_EXIT,
    RUNTIME_FIXED_ARRAY_FIELD_GUARD_EXIT,
    RUNTIME_FIXED_ARRAY_FIELD_VALUE_EXIT,
    FIXED_ARRAY_ELEMENT_GUARD,
];

pub(crate) const FAIL_CANARIES: &[&str] = &[TERMINATION_SLICE_LENGTH_ORDER_UNIMPLEMENTED];
