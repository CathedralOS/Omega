//! Fixture identities shared with the executing owner and corpus inventory.

pub const RUNTIME_FLOAT_LOCAL_ARITHMETIC_EXIT: &str =
    "expressions/runtime_float_local_arithmetic_exit";
pub const FLOAT_ARRAY_BINARY_OP_ZERO: &str = "expressions/float_array_binary_op_zero";
pub const F32_ARRAY_BINARY_OP_ZERO: &str = "expressions/f32_array_binary_op_zero";
pub const ARITHMETIC_DOMAIN_WRAPPING_EXIT: &str = "expressions/arithmetic_domain_wrapping_exit";
pub const ARITHMETIC_DOMAIN_SATURATING_EXIT: &str = "expressions/arithmetic_domain_saturating_exit";
pub const ARITHMETIC_DOMAIN_SATURATING_DIV_MOD_EXIT: &str =
    "expressions/arithmetic_domain_saturating_div_mod_exit";
pub const RUNTIME_GUARD_DIVIDE_MODULO_EXIT: &str = "expressions/runtime_guard_divide_modulo_exit";
pub const RUNTIME_GUARD_NEGATIVE_ARITHMETIC_EXIT: &str =
    "expressions/runtime_guard_negative_arithmetic_exit";
pub const RUNTIME_GUARD_DIVIDE_MODULO_SIGNEDNESS_EXIT: &str =
    "expressions/runtime_guard_divide_modulo_signedness_exit";
pub const RUNTIME_NESTED_LOOP_GRID_SUM_EXIT: &str =
    "control_flow/runtime_nested_loop_grid_sum_exit";
pub const RUNTIME_MULTI_FIELD_PAYLOAD_ARITH_EXIT: &str =
    "control_flow/runtime_multi_field_payload_arith_exit";
pub const CASE_PAYLOAD_SHARED_FIELD_NAME_EXIT: &str =
    "control_flow/case_payload_shared_field_name_exit";
pub const SUM_FIELD_STORAGE_ROUNDTRIP: &str = "control_flow/sum_field_storage_roundtrip";
pub const SUM_MIXED_WIDTH_PAYLOAD_LAYOUT: &str = "control_flow/sum_mixed_width_payload_layout";
pub const ARITHMETIC_DOMAIN_SATURATING_MUL_EXIT: &str =
    "expressions/arithmetic_domain_saturating_mul_exit";
pub const ARITHMETIC_DOMAIN_SATURATING_MUL_SIGNED_EXIT: &str =
    "expressions/arithmetic_domain_saturating_mul_signed_exit";
pub const ARITHMETIC_DOMAIN_TRAPPING_DIV_EXIT: &str =
    "expressions/arithmetic_domain_trapping_div_exit";
pub const ARITHMETIC_DOMAIN_TRAPPING_MUL_EXIT: &str =
    "expressions/arithmetic_domain_trapping_mul_exit";
pub const RUNTIME_TRANSITION_ARG_GUARD_NARROWING_EXIT: &str =
    "arithmetic/runtime_transition_arg_guard_narrowing_exit";
pub const RUNTIME_REQUIRES_ONE_SIDED_BOUND_EXIT: &str =
    "arithmetic/runtime_requires_one_sided_bound_exit";
pub const RUNTIME_TRANSITION_VALUE_GUARD_NARROWING_EXIT: &str =
    "arithmetic/runtime_transition_value_guard_narrowing_exit";
pub const RUNTIME_TRANSITION_ARG_FALSE_ARM_NARROWING_EXIT: &str =
    "arithmetic/runtime_transition_arg_false_arm_narrowing_exit";
pub const RUNTIME_TRANSITION_ARG_SATURATING_EXIT: &str =
    "arithmetic/runtime_transition_arg_saturating_exit";
pub const RUNTIME_CAST_ELEMENT_ACCUMULATOR_EXIT: &str =
    "arithmetic/runtime_cast_element_accumulator_exit";
pub const RUNTIME_DOMAIN_BOUNDARIES_EXIT: &str = "arithmetic/runtime_domain_boundaries_exit";
pub const RUNTIME_COMPARISON_SIGNEDNESS_EXIT: &str =
    "arithmetic/runtime_comparison_signedness_exit";
pub const RUNTIME_SHIFT_SIGNEDNESS_EXIT: &str = "arithmetic/runtime_shift_signedness_exit";
pub const RUNTIME_SHIFT_IN_GUARD_EXIT: &str = "arithmetic/runtime_shift_in_guard_exit";
pub const RUNTIME_CAST_IN_GUARD_EXIT: &str = "arithmetic/runtime_cast_in_guard_exit";
pub const RUNTIME_PARENTHESIZED_GUARD_SUBJECTS_EXIT: &str =
    "arithmetic/runtime_parenthesized_guard_subjects_exit";
pub const RUNTIME_AND_OF_OR_GUARD_EXIT: &str = "arithmetic/runtime_and_of_or_guard_exit";
pub const RUNTIME_NEGATED_BOOLEAN_NESTING_GUARD_EXIT: &str =
    "arithmetic/runtime_negated_boolean_nesting_guard_exit";
pub const RUNTIME_GUARD_FEATURE_COMPOSITION_EXIT: &str =
    "arithmetic/runtime_guard_feature_composition_exit";
pub const RUNTIME_SATURATING_NARROW_ADD_SUB_EXIT: &str =
    "arithmetic/runtime_saturating_narrow_add_sub_exit";
pub const RUNTIME_UNSIGNED_HIGH_BIT_U32_OPS_EXIT: &str =
    "arithmetic/runtime_unsigned_high_bit_u32_ops_exit";
pub const RUNTIME_NARROW_SIGNED_WRAP_BOUNDARIES_EXIT: &str =
    "arithmetic/runtime_narrow_signed_wrap_boundaries_exit";
pub const RUNTIME_NARROW_SIGNED_GUARD_OPS_EXIT: &str =
    "arithmetic/runtime_narrow_signed_guard_ops_exit";
pub const RUNTIME_NARROW_SIGNED_DIVIDE_GUARD_EXIT: &str =
    "arithmetic/runtime_narrow_signed_divide_guard_exit";
pub const RUNTIME_SATURATING_NARROW_DIVIDE_EXIT: &str =
    "arithmetic/runtime_saturating_narrow_divide_exit";
pub const RUNTIME_MIXED_WIDTH_SIGN_EXIT: &str = "arithmetic/runtime_mixed_width_sign_exit";
pub const RUNTIME_INTEGER_CASTS_EXIT: &str = "arithmetic/runtime_integer_casts_exit";
pub const RUNTIME_I64_DIVIDE_MODULO_EXIT: &str = "arithmetic/runtime_i64_divide_modulo_exit";
pub const RUNTIME_FLOAT_COMPARE_CAST_EXIT: &str = "arithmetic/runtime_float_compare_cast_exit";
pub const RUNTIME_FLOAT_OPERATIONS_EXIT: &str = "arithmetic/runtime_float_operations_exit";
pub const RUNTIME_INFERRED_MULTIPATH_RETURN_EXIT: &str =
    "arithmetic/runtime_inferred_multipath_return_exit";
pub const RUNTIME_INFERRED_RETURN_RANGE_EXIT: &str =
    "arithmetic/runtime_inferred_return_range_exit";
pub const RUNTIME_PROVABLE_FIELD_CONSTRUCTION_EXIT: &str =
    "arithmetic/runtime_provable_field_construction_exit";
pub const RUNTIME_STRUCT_FIELD_RANGE_NARROWING_EXIT: &str =
    "arithmetic/runtime_struct_field_range_narrowing_exit";
pub const RUNTIME_PAYLOAD_RANGE_NARROWING_EXIT: &str =
    "arithmetic/runtime_payload_range_narrowing_exit";
pub const SUM_PAYLOAD_RANGE_NARROWED_EXIT: &str = "ranges/sum_payload_range_narrowed_exit";
pub const SUM_PAYLOAD_RANGE_ARITH_NARROWED_EXIT: &str =
    "ranges/sum_payload_range_arith_narrowed_exit";
pub const RUNTIME_EXCLUSIVE_RANGE_CONSTRAINT_EXIT: &str =
    "arithmetic/runtime_exclusive_range_constraint_exit";
pub const RUNTIME_FNV1A_HASH_EXIT: &str = "arithmetic/runtime_fnv1a_hash_exit";
pub const RUNTIME_MIN_MAX_CLAMP_NARROWING_EXIT: &str =
    "arithmetic/runtime_min_max_clamp_narrowing_exit";
pub const RUNTIME_MODULO_DIV_NARROWING_EXIT: &str = "arithmetic/runtime_modulo_div_narrowing_exit";
pub const ARITHMETIC_DOMAIN_TRAPPING_MUL_OVERFLOW: &str =
    "expressions/arithmetic_domain_trapping_mul_overflow";
pub const ARITHMETIC_DOMAIN_SATURATING_SIGNED_EXIT: &str =
    "expressions/arithmetic_domain_saturating_signed_exit";
pub const ARITHMETIC_DOMAIN_REQUIRES_PROVEN_EXACT_EXIT: &str =
    "expressions/arithmetic_domain_requires_proven_exact_exit";
pub const ARITHMETIC_DOMAIN_RANGE_PROVEN_EXACT_EXIT: &str =
    "expressions/arithmetic_domain_range_proven_exact_exit";
pub const ARITHMETIC_DOMAIN_CAST_EXIT: &str = "expressions/arithmetic_domain_cast_exit";
pub const ARITHMETIC_DOMAIN_TRAPPING_EXIT: &str = "expressions/arithmetic_domain_trapping_exit";
pub const ARITHMETIC_DOMAIN_TRAPPING_OVERFLOW: &str =
    "expressions/arithmetic_domain_trapping_overflow";
pub const ARITHMETIC_DOMAIN_TRAPPING_LET_OVERFLOW: &str =
    "expressions/arithmetic_domain_trapping_let_overflow";
pub const ARITHMETIC_DOMAIN_RETURN_RANGE_PROVEN_EXACT_EXIT: &str =
    "expressions/arithmetic_domain_return_range_proven_exact_exit";
pub const ARITHMETIC_DOMAIN_TRAPPING_CONST_FOLD_OVERFLOW: &str =
    "expressions/arithmetic_domain_trapping_const_fold_overflow";
pub const CONSTANT_TRAPPING_SHIFT_VALUE_OVERFLOW_TRAPS: &str =
    "arithmetic/constant_trapping_shift_value_overflow_traps";
pub const DEAD_TRAPPING_LET_TRAPS: &str = "expressions/dead_trapping_let_traps";
pub const F32_FIELD_BINARY_TO_LOCAL_CAST: &str = "expressions/f32_field_binary_to_local_cast";
pub const F32_TO_F64_LOCAL_CAST: &str = "expressions/f32_to_f64_local_cast";
pub const F32_DEEP_CHAIN_BINARY: &str = "expressions/f32_deep_chain_binary";
pub const NO_PAYLOAD_CASE_VARIANT_AFTER_PAYLOAD_DISPATCH_EXIT: &str =
    "control_flow/no_payload_case_variant_after_payload_dispatch_exit";
pub const TRANSITION_ARG_LOCAL_FROM_EMBEDDED_CALL_EXIT: &str =
    "calls/transition_arg_local_from_embedded_call_exit";
pub const VALUE_CALL_EMBEDDED_IN_BINARY_EXIT: &str = "calls/value_call_embedded_in_binary_exit";
pub const SEQUENTIAL_SELF_FIELD_RMW_EXIT: &str = "calls/sequential_self_field_rmw_exit";
pub const RUNTIME_LITERAL_SOURCE_CAST_EXIT: &str = "expressions/runtime_literal_source_cast_exit";
pub const RUNTIME_FLOAT_CONSTANT_STORE_EXIT: &str = "expressions/runtime_float_constant_store_exit";
pub const RUNTIME_MATCH_VALUE_EXIT: &str = "expressions/runtime_match_value_exit";
pub const RUNTIME_FLAT_BOOLEAN_LOGIC_EXIT: &str = "expressions/runtime_flat_boolean_logic_exit";
pub const RUNTIME_ENUM_MATCH_BREADTH_EXIT: &str = "expressions/runtime_enum_match_breadth_exit";
pub const RUNTIME_CONFORMANCE_ITEM_EXIT: &str = "traits/runtime_conformance_item_exit";
pub const EQUATABLE_RECORD_EQUALITY_EXIT: &str = "traits/equatable_record_equality_exit";
pub const EQUATABLE_SUM_PAYLOAD_EQUALITY_EXIT: &str = "traits/equatable_sum_payload_equality_exit";
pub const EQUATABLE_MIXED_SHAPE_EQUALITY_EXIT: &str = "traits/equatable_mixed_shape_equality_exit";
pub const EQUATABLE_STRING_FIELD_EQUALITY_EXIT: &str =
    "traits/equatable_string_field_equality_exit";
pub const EQUATABLE_STRING_NOT_EQUALS_EXIT: &str = "traits/equatable_string_not_equals_exit";
pub const EQUATABLE_STRING_EQUALITY_GUARD_EXIT: &str =
    "traits/equatable_string_equality_guard_exit";
pub const RUNTIME_DEEP_NESTED_FIELD_EXIT: &str = "data/runtime_deep_nested_field_exit";
pub const RUNTIME_STRUCT_VALUE_COPY_EXIT: &str = "data/runtime_struct_value_copy_exit";
pub const RUNTIME_WHOLE_STRUCT_MUTATION_COPY_EXIT: &str =
    "data/runtime_whole_struct_mutation_copy_exit";
pub const RUNTIME_DATA_PROPERTIES_EXIT: &str = "data/runtime_data_properties_exit";
pub const CASE_FIRST_PAYLOAD_ZERO_ESTABLISHED: &str = "data/case_first_payload_zero_established";
pub const COMPOUND_ASSIGNMENT_EXIT: &str = "operators/compound_assignment_exit";
pub const RUNTIME_CHAINED_FIELD_MUTATION_EXIT: &str =
    "arithmetic/runtime_chained_field_mutation_exit";
pub const RUNTIME_COMPARISON_GUARD_SIGNEDNESS_EXIT: &str =
    "arithmetic/runtime_comparison_guard_signedness_exit";
pub const RUNTIME_COMPARISON_VALUE_SIGNEDNESS_EXIT: &str =
    "arithmetic/runtime_comparison_value_signedness_exit";
pub const RUNTIME_MIN_MAX_SIGNEDNESS_EXIT: &str = "arithmetic/runtime_min_max_signedness_exit";
pub const RUNTIME_UNSIGNED_DIVISION_EXIT: &str = "arithmetic/runtime_unsigned_division_exit";
pub const RUNTIME_UNSIGNED_MIN_MAX_EXIT: &str = "arithmetic/runtime_unsigned_min_max_exit";
pub const RUNTIME_UNSIGNED_MODULO_CALL_ARGUMENT_EXIT: &str =
    "arithmetic/runtime_unsigned_modulo_call_argument_exit";
pub const RUNTIME_NESTED_NAMED_CONVERSION_ALIAS_EXIT: &str =
    "calls/runtime_nested_named_conversion_alias_exit";
pub const RUNTIME_UNSIGNED_MODULO_CAST_OPERAND_EXIT: &str =
    "arithmetic/runtime_unsigned_modulo_cast_operand_exit";
pub const SATURATING_MULTIPLY_OVERFLOW_BOTH_SIGNS: &str =
    "arithmetic/saturating_multiply_overflow_both_signs";
pub const SATURATING_SIGNED_DIVIDE_MIN_BY_NEG_ONE: &str =
    "arithmetic/saturating_signed_divide_min_by_neg_one";
pub const WRAPPING_SIGNED_DIVIDE_MIN_BY_NEG_ONE: &str =
    "arithmetic/wrapping_signed_divide_min_by_neg_one";
pub const RUNTIME_SIGNED_DIVISION_EXIT: &str = "arithmetic/runtime_signed_division_exit";
pub const RUNTIME_SHIFT_RIGHT_SIGNEDNESS: &str = "arithmetic/runtime_shift_right_signedness";
pub const CONST_FOLD_SATURATING_NARROW_EXIT: &str = "arithmetic/const_fold_saturating_narrow_exit";
pub const CONST_FOLD_WRAPPING_NARROW_EXIT: &str = "arithmetic/const_fold_wrapping_narrow_exit";

pub const PASS_CANARIES: &[&str] = &[
    RUNTIME_FLOAT_LOCAL_ARITHMETIC_EXIT,
    FLOAT_ARRAY_BINARY_OP_ZERO,
    F32_ARRAY_BINARY_OP_ZERO,
    ARITHMETIC_DOMAIN_WRAPPING_EXIT,
    ARITHMETIC_DOMAIN_SATURATING_EXIT,
    ARITHMETIC_DOMAIN_SATURATING_DIV_MOD_EXIT,
    RUNTIME_GUARD_DIVIDE_MODULO_EXIT,
    RUNTIME_GUARD_NEGATIVE_ARITHMETIC_EXIT,
    RUNTIME_GUARD_DIVIDE_MODULO_SIGNEDNESS_EXIT,
    RUNTIME_NESTED_LOOP_GRID_SUM_EXIT,
    RUNTIME_MULTI_FIELD_PAYLOAD_ARITH_EXIT,
    CASE_PAYLOAD_SHARED_FIELD_NAME_EXIT,
    SUM_FIELD_STORAGE_ROUNDTRIP,
    SUM_MIXED_WIDTH_PAYLOAD_LAYOUT,
    ARITHMETIC_DOMAIN_SATURATING_MUL_EXIT,
    ARITHMETIC_DOMAIN_SATURATING_MUL_SIGNED_EXIT,
    ARITHMETIC_DOMAIN_TRAPPING_DIV_EXIT,
    ARITHMETIC_DOMAIN_TRAPPING_MUL_EXIT,
    RUNTIME_TRANSITION_ARG_GUARD_NARROWING_EXIT,
    RUNTIME_REQUIRES_ONE_SIDED_BOUND_EXIT,
    RUNTIME_TRANSITION_VALUE_GUARD_NARROWING_EXIT,
    RUNTIME_TRANSITION_ARG_FALSE_ARM_NARROWING_EXIT,
    RUNTIME_TRANSITION_ARG_SATURATING_EXIT,
    RUNTIME_CAST_ELEMENT_ACCUMULATOR_EXIT,
    RUNTIME_DOMAIN_BOUNDARIES_EXIT,
    RUNTIME_COMPARISON_SIGNEDNESS_EXIT,
    RUNTIME_SHIFT_SIGNEDNESS_EXIT,
    RUNTIME_SHIFT_IN_GUARD_EXIT,
    RUNTIME_CAST_IN_GUARD_EXIT,
    RUNTIME_PARENTHESIZED_GUARD_SUBJECTS_EXIT,
    RUNTIME_AND_OF_OR_GUARD_EXIT,
    RUNTIME_NEGATED_BOOLEAN_NESTING_GUARD_EXIT,
    RUNTIME_GUARD_FEATURE_COMPOSITION_EXIT,
    RUNTIME_SATURATING_NARROW_ADD_SUB_EXIT,
    RUNTIME_UNSIGNED_HIGH_BIT_U32_OPS_EXIT,
    RUNTIME_NARROW_SIGNED_WRAP_BOUNDARIES_EXIT,
    RUNTIME_NARROW_SIGNED_GUARD_OPS_EXIT,
    RUNTIME_NARROW_SIGNED_DIVIDE_GUARD_EXIT,
    RUNTIME_SATURATING_NARROW_DIVIDE_EXIT,
    RUNTIME_MIXED_WIDTH_SIGN_EXIT,
    RUNTIME_INTEGER_CASTS_EXIT,
    RUNTIME_I64_DIVIDE_MODULO_EXIT,
    RUNTIME_FLOAT_COMPARE_CAST_EXIT,
    RUNTIME_FLOAT_OPERATIONS_EXIT,
    RUNTIME_INFERRED_MULTIPATH_RETURN_EXIT,
    RUNTIME_INFERRED_RETURN_RANGE_EXIT,
    RUNTIME_PROVABLE_FIELD_CONSTRUCTION_EXIT,
    RUNTIME_STRUCT_FIELD_RANGE_NARROWING_EXIT,
    RUNTIME_PAYLOAD_RANGE_NARROWING_EXIT,
    SUM_PAYLOAD_RANGE_NARROWED_EXIT,
    SUM_PAYLOAD_RANGE_ARITH_NARROWED_EXIT,
    RUNTIME_EXCLUSIVE_RANGE_CONSTRAINT_EXIT,
    RUNTIME_FNV1A_HASH_EXIT,
    RUNTIME_MIN_MAX_CLAMP_NARROWING_EXIT,
    RUNTIME_MODULO_DIV_NARROWING_EXIT,
    ARITHMETIC_DOMAIN_TRAPPING_MUL_OVERFLOW,
    ARITHMETIC_DOMAIN_SATURATING_SIGNED_EXIT,
    ARITHMETIC_DOMAIN_REQUIRES_PROVEN_EXACT_EXIT,
    ARITHMETIC_DOMAIN_RANGE_PROVEN_EXACT_EXIT,
    ARITHMETIC_DOMAIN_CAST_EXIT,
    ARITHMETIC_DOMAIN_TRAPPING_EXIT,
    ARITHMETIC_DOMAIN_TRAPPING_OVERFLOW,
    ARITHMETIC_DOMAIN_TRAPPING_LET_OVERFLOW,
    ARITHMETIC_DOMAIN_RETURN_RANGE_PROVEN_EXACT_EXIT,
    ARITHMETIC_DOMAIN_TRAPPING_CONST_FOLD_OVERFLOW,
    CONSTANT_TRAPPING_SHIFT_VALUE_OVERFLOW_TRAPS,
    DEAD_TRAPPING_LET_TRAPS,
    F32_FIELD_BINARY_TO_LOCAL_CAST,
    F32_TO_F64_LOCAL_CAST,
    F32_DEEP_CHAIN_BINARY,
    NO_PAYLOAD_CASE_VARIANT_AFTER_PAYLOAD_DISPATCH_EXIT,
    TRANSITION_ARG_LOCAL_FROM_EMBEDDED_CALL_EXIT,
    VALUE_CALL_EMBEDDED_IN_BINARY_EXIT,
    SEQUENTIAL_SELF_FIELD_RMW_EXIT,
    RUNTIME_LITERAL_SOURCE_CAST_EXIT,
    RUNTIME_FLOAT_CONSTANT_STORE_EXIT,
    RUNTIME_MATCH_VALUE_EXIT,
    RUNTIME_FLAT_BOOLEAN_LOGIC_EXIT,
    RUNTIME_ENUM_MATCH_BREADTH_EXIT,
    RUNTIME_CONFORMANCE_ITEM_EXIT,
    EQUATABLE_RECORD_EQUALITY_EXIT,
    EQUATABLE_SUM_PAYLOAD_EQUALITY_EXIT,
    EQUATABLE_MIXED_SHAPE_EQUALITY_EXIT,
    EQUATABLE_STRING_FIELD_EQUALITY_EXIT,
    EQUATABLE_STRING_NOT_EQUALS_EXIT,
    EQUATABLE_STRING_EQUALITY_GUARD_EXIT,
    RUNTIME_DEEP_NESTED_FIELD_EXIT,
    RUNTIME_STRUCT_VALUE_COPY_EXIT,
    RUNTIME_WHOLE_STRUCT_MUTATION_COPY_EXIT,
    RUNTIME_DATA_PROPERTIES_EXIT,
    CASE_FIRST_PAYLOAD_ZERO_ESTABLISHED,
    COMPOUND_ASSIGNMENT_EXIT,
    RUNTIME_CHAINED_FIELD_MUTATION_EXIT,
    RUNTIME_COMPARISON_GUARD_SIGNEDNESS_EXIT,
    RUNTIME_COMPARISON_VALUE_SIGNEDNESS_EXIT,
    RUNTIME_MIN_MAX_SIGNEDNESS_EXIT,
    RUNTIME_UNSIGNED_DIVISION_EXIT,
    RUNTIME_UNSIGNED_MIN_MAX_EXIT,
    RUNTIME_UNSIGNED_MODULO_CALL_ARGUMENT_EXIT,
    RUNTIME_NESTED_NAMED_CONVERSION_ALIAS_EXIT,
    RUNTIME_UNSIGNED_MODULO_CAST_OPERAND_EXIT,
    SATURATING_MULTIPLY_OVERFLOW_BOTH_SIGNS,
    SATURATING_SIGNED_DIVIDE_MIN_BY_NEG_ONE,
    WRAPPING_SIGNED_DIVIDE_MIN_BY_NEG_ONE,
    RUNTIME_SIGNED_DIVISION_EXIT,
    RUNTIME_SHIFT_RIGHT_SIGNEDNESS,
    CONST_FOLD_SATURATING_NARROW_EXIT,
    CONST_FOLD_WRAPPING_NARROW_EXIT,
];
