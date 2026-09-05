//! Exact fixture identities shared with the executing recursion/slice owner.

pub const RUNTIME_NAT_STRUCTURAL_RECURSION_EXIT: &str =
    "proofs/runtime_nat_structural_recursion_exit";
pub const RUNTIME_CORE_NAT_DECLARED_EXIT: &str = "proofs/runtime_core_nat_declared_exit";
pub const ACCEPTED_AXIOM_CITED_EXIT: &str = "proofs/accepted_axiom_cited_exit";
pub const RUNTIME_CORE_RAT_DECLARED_EXIT: &str = "proofs/runtime_core_rat_declared_exit";
pub const RUNTIME_FREE_CONST_EXIT: &str = "constants/runtime_free_const_exit";
pub const RUNTIME_VALUE_CALL_TERMINAL_EXIT: &str = "calls/runtime_value_call_terminal_exit";
pub const RUNTIME_RESULT_DOMAIN_MACHINE_OVERLOAD_EXIT: &str =
    "domains/runtime_result_domain_machine_overload_exit";
pub const RUNTIME_STD_MATH_SIN_COS_EXIT: &str = "calls/runtime_std_math_sin_cos_exit";
pub const RUNTIME_COMPUTED_INDEX_MATCH_SUBJECT_EXIT: &str =
    "collections/runtime_computed_index_match_subject_exit";
pub const RUNTIME_CONST_MEASURED_RECURSION_EXIT: &str =
    "comptime/runtime_const_measured_recursion_exit";
pub const RUNTIME_TERMINAL_TAIL_RECURSION_EXIT: &str = "calls/runtime_terminal_tail_recursion_exit";
pub const RUNTIME_MEASURED_TAIL_RECURSION_EXIT: &str = "calls/runtime_measured_tail_recursion_exit";
pub const RUNTIME_U64_GUARDED_CAP_STORE_EXIT: &str =
    "arithmetic/runtime_u64_guarded_cap_store_exit";
pub const RUNTIME_PROOF_ONLY_DATA_DECLARED_EXIT: &str =
    "data/runtime_proof_only_data_declared_exit";
pub const RUNTIME_F32_FIELD_GUARD_EXIT: &str = "arithmetic/runtime_f32_field_guard_exit";
pub const RUNTIME_COMPUTED_ARRAY_FILL_VIA_TEMP_EXIT: &str =
    "collections/runtime_computed_array_fill_via_temp_exit";
pub const RUNTIME_NESTED_LOOP_FILL_EXIT: &str = "collections/runtime_nested_loop_fill_exit";
pub const RUNTIME_LOOP_COUNTER_INIT_HOISTED_EXIT: &str =
    "collections/runtime_loop_counter_init_hoisted_exit";
pub const RUNTIME_WRITE_FIRST_LOOP_INDEX_EXIT: &str =
    "collections/runtime_write_first_loop_index_exit";
pub const RUNTIME_ARRAY_INDEXED_LOOP_EXIT: &str = "slices/runtime_array_indexed_loop_exit";
pub const RUNTIME_DECREASING_INDEX_EXIT: &str = "slices/runtime_decreasing_index_exit";
pub const RUNTIME_SLICE_INDEXED_READ_EXIT: &str = "slices/runtime_slice_indexed_read_exit";
pub const RUNTIME_ARRAY_ADJACENT_INDEX_EXIT: &str = "slices/runtime_array_adjacent_index_exit";
pub const RUNTIME_NESTED_DECREASING_INDEX_EXIT: &str =
    "slices/runtime_nested_decreasing_index_exit";
pub const RUNTIME_NARROW_WIDEN_CAST_EXIT: &str = "slices/runtime_narrow_widen_cast_exit";
pub const RUNTIME_SIGNED_INDEX_GUARDED_EXIT: &str = "slices/runtime_signed_index_guarded_exit";
pub const RUNTIME_TWO_POINTER_SUM_EXIT: &str = "slices/runtime_two_pointer_sum_exit";
pub const RUNTIME_TWO_POINTER_REVERSE_EXIT: &str = "slices/runtime_two_pointer_reverse_exit";
pub const RUNTIME_BRANCHED_INDEX_BOUND_EXIT: &str = "slices/runtime_branched_index_bound_exit";
pub const RUNTIME_INDEXED_ARRAY_WRITE_EXIT: &str = "slices/runtime_indexed_array_write_exit";
pub const RECURSIVE_SUBSLICE_ELEMENT_ACCUMULATOR_EXIT: &str =
    "slices/recursive_subslice_element_accumulator_exit";
pub const RUNTIME_SUBSLICE_OF_SLICE_PARAM_EXIT: &str =
    "slices/runtime_subslice_of_slice_param_exit";
pub const RUNTIME_MACHINE_FIELD_SUBSLICE_ARG_INDEX_EXIT: &str =
    "slices/runtime_machine_field_subslice_arg_index_exit";
pub const RUNTIME_SLICE_INDEX_READ_EXIT: &str = "slices/runtime_slice_index_read_exit";
pub const RUNTIME_INDEXED_READ_OPERAND_EXIT: &str = "slices/runtime_indexed_read_operand_exit";
pub const NUMERIC_CONVERSION_SURFACE: &str = "core/numeric_conversion_surface";
pub const RUNTIME_I64_TO_U64_EXACT_GUARD_EXIT: &str =
    "arithmetic/runtime_i64_to_u64_exact_guard_exit";
pub const NUMERIC_SIGNED_CONVERSION_SURFACE: &str = "core/numeric_signed_conversion_surface";
pub const NUMERIC_TRAPPING_CONVERSION_OVERFLOW: &str = "core/numeric_trapping_conversion_overflow";
pub const NUMERIC_CROSS_SIGNED_CONVERSION_SURFACE: &str =
    "core/numeric_cross_signed_conversion_surface";
pub const RUNTIME_SUBSLICE_LEN_EXIT: &str = "slices/runtime_subslice_len_exit";
pub const RUNTIME_SLICE_INDEX_READ_DISPATCH_EXIT: &str =
    "slices/runtime_slice_index_read_dispatch_exit";
pub const RUNTIME_SLICE_INDEX_COPY_EXIT: &str = "slices/runtime_slice_index_copy_exit";
pub const RUNTIME_SLICE_INDEX_COPY_DISPATCH_EXIT: &str =
    "slices/runtime_slice_index_copy_dispatch_exit";
pub const RUNTIME_FRAME_ARRAY_SLICE_PARAMETER_ALIAS_EXIT: &str =
    "slices/runtime_frame_array_slice_parameter_alias_exit";
pub const RUNTIME_SLICE_LEN_TRANSITION_EXIT: &str = "slices/runtime_slice_len_transition_exit";
pub const RUNTIME_SUBSLICE_PARAM_BOUNDED_RANGE_EXIT: &str =
    "slices/runtime_subslice_param_bounded_range_exit";
pub const RUNTIME_SUBSLICE_PARAM_END_ONLY_EXIT: &str =
    "slices/runtime_subslice_param_end_only_exit";
pub const RUNTIME_SUBSLICE_PARAM_LOCAL_EXIT: &str = "slices/runtime_subslice_param_local_exit";
pub const RUNTIME_SUBSLICE_RUNTIME_START_EXIT: &str = "slices/runtime_subslice_runtime_start_exit";
pub const RUNTIME_SUBSLICE_RUNTIME_END_EXIT: &str = "slices/runtime_subslice_runtime_end_exit";
pub const RUNTIME_SUBSLICE_NESTED_OF_PARAM_EXIT: &str =
    "slices/runtime_subslice_nested_of_param_exit";
pub const RUNTIME_SUBSLICE_RUNTIME_START_OVER_LOCAL_EXIT: &str =
    "slices/runtime_subslice_runtime_start_over_local_exit";
pub const RUNTIME_SUBSLICE_PARAM_INCLUSIVE_END_EXIT: &str =
    "slices/runtime_subslice_param_inclusive_end_exit";
pub const RUNTIME_SUBSLICE_RANGE_LEN_EXIT: &str = "slices/runtime_subslice_range_len_exit";
pub const RUNTIME_SUBSLICE_BOUNDED_RANGE_LEN_EXIT: &str =
    "slices/runtime_subslice_bounded_range_len_exit";
pub const RUNTIME_SUBSLICE_RANGE_POINTER_EXIT: &str = "slices/runtime_subslice_range_pointer_exit";
pub const RUNTIME_LOCAL_AGGREGATE_INTO_LET_EXIT: &str =
    "slices/runtime_local_aggregate_into_let_exit";
pub const RUNTIME_FIELD_ARRAY_ELEMENT_VALUE_OPERAND_EXIT: &str =
    "slices/runtime_field_array_element_value_operand_exit";
pub const RUNTIME_SUBSLICE_DYNAMIC_INDEX_EXIT: &str = "slices/runtime_subslice_dynamic_index_exit";
pub const RUNTIME_SUBSLICE_BOUNDED_DYNAMIC_INDEX_EXIT: &str =
    "slices/runtime_subslice_bounded_dynamic_index_exit";
pub const RUNTIME_SUBSLICE_END_DYNAMIC_INDEX_EXIT: &str =
    "slices/runtime_subslice_end_dynamic_index_exit";
pub const RUNTIME_NESTED_SUBSLICE_DYNAMIC_INDEX_EXIT: &str =
    "slices/runtime_nested_subslice_dynamic_index_exit";
pub const RUNTIME_NESTED_SUBSLICE_FIXED_INDEX_EXIT: &str =
    "slices/runtime_nested_subslice_fixed_index_exit";
pub const RUNTIME_SLICE_FIXED_INDEX_GUARD_EXIT: &str =
    "slices/runtime_slice_fixed_index_guard_exit";
pub const RUNTIME_LOCAL_SLICE_LEN_COMPARISON_VALUE_EXIT: &str =
    "slices/runtime_local_slice_len_comparison_value_exit";
pub const RUNTIME_SLICE_INDEX_TRANSITION_EXIT: &str = "slices/runtime_slice_index_transition_exit";
pub const RUNTIME_SLICE_ITERATION_EXIT: &str = "slices/runtime_slice_iteration_exit";
pub const RUNTIME_STRING_CONCAT_MEMBERSHIP_EXIT: &str =
    "text/runtime_string_concat_membership_exit";
pub const RUNTIME_STRING_FIELD_CONCAT_EXIT: &str = "text/runtime_string_field_concat_exit";
pub const RUNTIME_MACHINE_OWNED_INDEXED_STRING_FIELD_CONCAT_EXIT: &str =
    "text/runtime_machine_owned_indexed_string_field_concat_exit";
pub const RUNTIME_MACHINE_OWNED_INDEXED_BOUNDED_CARRIER_LITERAL_EXIT: &str =
    "text/runtime_machine_owned_indexed_bounded_carrier_literal_exit";
pub const RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_BOUNDED_CARRIER_LITERAL_EXIT: &str =
    "text/runtime_machine_owned_double_indexed_bounded_carrier_literal_exit";
pub const RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_STRING_FIELD_CONCAT_EXIT: &str =
    "text/runtime_machine_owned_double_indexed_string_field_concat_exit";
pub const RUNTIME_MUTABLE_MACHINE_OWNED_PARAMETER_WRITE_EXIT: &str =
    "calls/runtime_mutable_machine_owned_parameter_write_exit";
pub const RUNTIME_MUTABLE_LOCAL_PARAMETER_WRITE_EXIT: &str =
    "calls/runtime_mutable_local_parameter_write_exit";
pub const RUNTIME_MUTABLE_PARAMETER_READ_MODIFY_WRITE_EXIT: &str =
    "calls/runtime_mutable_parameter_read_modify_write_exit";

pub const PASS_CANARIES: &[&str] = &[
    RUNTIME_NAT_STRUCTURAL_RECURSION_EXIT,
    RUNTIME_CORE_NAT_DECLARED_EXIT,
    ACCEPTED_AXIOM_CITED_EXIT,
    RUNTIME_CORE_RAT_DECLARED_EXIT,
    RUNTIME_FREE_CONST_EXIT,
    RUNTIME_VALUE_CALL_TERMINAL_EXIT,
    RUNTIME_RESULT_DOMAIN_MACHINE_OVERLOAD_EXIT,
    RUNTIME_STD_MATH_SIN_COS_EXIT,
    RUNTIME_COMPUTED_INDEX_MATCH_SUBJECT_EXIT,
    RUNTIME_CONST_MEASURED_RECURSION_EXIT,
    RUNTIME_TERMINAL_TAIL_RECURSION_EXIT,
    RUNTIME_MEASURED_TAIL_RECURSION_EXIT,
    RUNTIME_U64_GUARDED_CAP_STORE_EXIT,
    RUNTIME_PROOF_ONLY_DATA_DECLARED_EXIT,
    RUNTIME_F32_FIELD_GUARD_EXIT,
    RUNTIME_COMPUTED_ARRAY_FILL_VIA_TEMP_EXIT,
    RUNTIME_NESTED_LOOP_FILL_EXIT,
    RUNTIME_LOOP_COUNTER_INIT_HOISTED_EXIT,
    RUNTIME_WRITE_FIRST_LOOP_INDEX_EXIT,
    RUNTIME_ARRAY_INDEXED_LOOP_EXIT,
    RUNTIME_DECREASING_INDEX_EXIT,
    RUNTIME_SLICE_INDEXED_READ_EXIT,
    RUNTIME_ARRAY_ADJACENT_INDEX_EXIT,
    RUNTIME_NESTED_DECREASING_INDEX_EXIT,
    RUNTIME_NARROW_WIDEN_CAST_EXIT,
    RUNTIME_SIGNED_INDEX_GUARDED_EXIT,
    RUNTIME_TWO_POINTER_SUM_EXIT,
    RUNTIME_TWO_POINTER_REVERSE_EXIT,
    RUNTIME_BRANCHED_INDEX_BOUND_EXIT,
    RUNTIME_INDEXED_ARRAY_WRITE_EXIT,
    RECURSIVE_SUBSLICE_ELEMENT_ACCUMULATOR_EXIT,
    RUNTIME_SUBSLICE_OF_SLICE_PARAM_EXIT,
    RUNTIME_MACHINE_FIELD_SUBSLICE_ARG_INDEX_EXIT,
    RUNTIME_SLICE_INDEX_READ_EXIT,
    RUNTIME_INDEXED_READ_OPERAND_EXIT,
    NUMERIC_CONVERSION_SURFACE,
    RUNTIME_I64_TO_U64_EXACT_GUARD_EXIT,
    NUMERIC_SIGNED_CONVERSION_SURFACE,
    NUMERIC_TRAPPING_CONVERSION_OVERFLOW,
    NUMERIC_CROSS_SIGNED_CONVERSION_SURFACE,
    RUNTIME_SUBSLICE_LEN_EXIT,
    RUNTIME_SLICE_INDEX_READ_DISPATCH_EXIT,
    RUNTIME_SLICE_INDEX_COPY_EXIT,
    RUNTIME_SLICE_INDEX_COPY_DISPATCH_EXIT,
    RUNTIME_FRAME_ARRAY_SLICE_PARAMETER_ALIAS_EXIT,
    RUNTIME_SLICE_LEN_TRANSITION_EXIT,
    RUNTIME_SUBSLICE_PARAM_BOUNDED_RANGE_EXIT,
    RUNTIME_SUBSLICE_PARAM_END_ONLY_EXIT,
    RUNTIME_SUBSLICE_PARAM_LOCAL_EXIT,
    RUNTIME_SUBSLICE_RUNTIME_START_EXIT,
    RUNTIME_SUBSLICE_RUNTIME_END_EXIT,
    RUNTIME_SUBSLICE_NESTED_OF_PARAM_EXIT,
    RUNTIME_SUBSLICE_RUNTIME_START_OVER_LOCAL_EXIT,
    RUNTIME_SUBSLICE_PARAM_INCLUSIVE_END_EXIT,
    RUNTIME_SUBSLICE_RANGE_LEN_EXIT,
    RUNTIME_SUBSLICE_BOUNDED_RANGE_LEN_EXIT,
    RUNTIME_SUBSLICE_RANGE_POINTER_EXIT,
    RUNTIME_LOCAL_AGGREGATE_INTO_LET_EXIT,
    RUNTIME_FIELD_ARRAY_ELEMENT_VALUE_OPERAND_EXIT,
    RUNTIME_SUBSLICE_DYNAMIC_INDEX_EXIT,
    RUNTIME_SUBSLICE_BOUNDED_DYNAMIC_INDEX_EXIT,
    RUNTIME_SUBSLICE_END_DYNAMIC_INDEX_EXIT,
    RUNTIME_NESTED_SUBSLICE_DYNAMIC_INDEX_EXIT,
    RUNTIME_NESTED_SUBSLICE_FIXED_INDEX_EXIT,
    RUNTIME_SLICE_FIXED_INDEX_GUARD_EXIT,
    RUNTIME_LOCAL_SLICE_LEN_COMPARISON_VALUE_EXIT,
    RUNTIME_SLICE_INDEX_TRANSITION_EXIT,
    RUNTIME_SLICE_ITERATION_EXIT,
    RUNTIME_STRING_CONCAT_MEMBERSHIP_EXIT,
    RUNTIME_STRING_FIELD_CONCAT_EXIT,
    RUNTIME_MACHINE_OWNED_INDEXED_STRING_FIELD_CONCAT_EXIT,
    RUNTIME_MACHINE_OWNED_INDEXED_BOUNDED_CARRIER_LITERAL_EXIT,
    RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_BOUNDED_CARRIER_LITERAL_EXIT,
    RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_STRING_FIELD_CONCAT_EXIT,
    RUNTIME_MUTABLE_MACHINE_OWNED_PARAMETER_WRITE_EXIT,
    RUNTIME_MUTABLE_LOCAL_PARAMETER_WRITE_EXIT,
    RUNTIME_MUTABLE_PARAMETER_READ_MODIFY_WRITE_EXIT,
];

pub const CROSS_SIGNED_TRAP_PASS_CANARIES: &[(&str, &str)] = &[
    (
        "core/numeric_cross_signed_unsigned_overflow_traps",
        "unsigned upper half to signed",
    ),
    (
        "core/numeric_cross_signed_negative_traps",
        "negative signed value to unsigned",
    ),
];
