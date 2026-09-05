//! Shared inputs for the generics and dependent-facts execution owner.
//! Execution tables preserve ordering and their inline/file diagnostic contracts.

pub(crate) const RUNTIME_DECREASES_U64_MEASURE_EXIT: &str =
    "proofs/runtime_decreases_u64_measure_exit";
pub(crate) const RUNTIME_WRAPPING_OPERAND_TRUNCATION_EXIT: &str =
    "arithmetic/runtime_wrapping_operand_truncation_exit";
pub(crate) const RUNTIME_FLOAT_COMPARE_BOOL_EXIT: &str =
    "arithmetic/runtime_float_compare_bool_exit";
pub(crate) const AGGREGATE_TRANSITION_ARGS_EXIT: &str = "structs/aggregate_transition_args_exit";
pub(crate) const DEEP_NESTED_WRITE_PATHS_EXIT: &str = "structs/deep_nested_write_paths_exit";
pub(crate) const ZII_DEFAULT_COMPOSITE_EXIT: &str = "core/zii_default_composite_exit";
pub(crate) const ZII_STRING_HOST_WRITE_EXIT: &str = "text/zii_string_host_write_exit";
pub(crate) const ZII_DEFAULT_STRING_EQUALITY_EXIT: &str = "text/zii_default_string_equality_exit";
pub(crate) const RUNTIME_OWNED_STRING_BYTE_VIEW_EXIT: &str =
    "text/runtime_owned_string_byte_view_exit";
pub(crate) const EQUATABLE_SUM_STALE_PAYLOAD_EXIT: &str = "traits/equatable_sum_stale_payload_exit";
pub(crate) const RUNTIME_TEXT_NOT_EQUALS_EXIT: &str = "text/runtime_text_not_equals_exit";
pub(crate) const RUNTIME_TEXT_EQUALS_BOOLEAN_OPERAND_EXIT: &str =
    "text/runtime_text_equals_boolean_operand_exit";
pub(crate) const CASE_LITERAL_TEXTEQ_TERMINAL_EXIT: &str = "text/case_literal_texteq_terminal_exit";
pub(crate) const CASE_LITERAL_TEXTEQ_FIELD_STORE_EXIT: &str =
    "text/case_literal_texteq_field_store_exit";
pub(crate) const RUNTIME_TEXT_EQUALS_VALUE_POSITIONS_EXIT: &str =
    "text/runtime_text_equals_value_positions_exit";
pub(crate) const SUM_PAYLOAD_CAST_OPERAND_FIELD_EXIT: &str =
    "control_flow/sum_payload_cast_operand_field_exit";
pub(crate) const RUNTIME_BRANCHING_CALLEE_CHAIN_EXIT: &str =
    "calls/runtime_branching_callee_chain_exit";
pub(crate) const RECURSIVE_RESULT_BIND_FIRST_ARG: &str = "calls/recursive_result_bind_first_arg";
pub(crate) const RUNTIME_RECURSIVE_RESULT_ROLES_EXIT: &str =
    "termination/runtime_recursive_result_roles_exit";
pub(crate) const RUNTIME_TRAPPING_GUARD_OVERFLOW_TRAPS: &str =
    "arithmetic/runtime_trapping_guard_overflow_traps";
pub(crate) const RUNTIME_TRAPPING_OVERFLOW_TRAPS: &str =
    "arithmetic/runtime_trapping_overflow_traps";
pub(crate) const RUNTIME_GUARD_PROVEN_COUNTER_EXIT: &str =
    "arithmetic/runtime_guard_proven_counter_exit";
pub(crate) const RUNTIME_GUARD_NARROWED_TRANSITION_ARG_EXIT: &str =
    "arithmetic/runtime_guard_narrowed_transition_arg_exit";
pub(crate) const RUNTIME_GUI_WINDOW_LIFECYCLE_EXIT: &str = "host/runtime_gui_window_lifecycle_exit";
pub(crate) const RUNTIME_GUI_FOREGROUND_WINDOW_EXIT: &str =
    "host/runtime_gui_foreground_window_exit";
pub(crate) const RUNTIME_GUI_WINDOW_BLIT_EXIT: &str = "host/runtime_gui_window_blit_exit";
pub(crate) const RUNTIME_GENERIC_VALUE_CALL_AGREEING_EXIT: &str =
    "generics/runtime_generic_value_call_agreeing_exit";
pub(crate) const RUNTIME_GENERIC_VALUE_CALL_EXIT: &str = "generics/runtime_generic_value_call_exit";
pub(crate) const TRAIT_GENERIC_BOUND_STATIC_DISPATCH: &str =
    "traits/trait_generic_bound_static_dispatch";
pub(crate) const RUNTIME_GENERIC_PARAM_POSITION_INFERENCE_EXIT: &str =
    "generics/runtime_generic_param_position_inference_exit";
pub(crate) const RUNTIME_GENERIC_MULTIPLE_SPECIALIZATIONS_EXIT: &str =
    "generics/runtime_generic_multiple_specializations_exit";
pub(crate) const RUNTIME_GENERIC_ENUM_PAYLOAD_EXIT: &str =
    "generics/runtime_generic_enum_payload_exit";
pub(crate) const RUNTIME_GENERIC_RECORD_INSTANCE_EXIT: &str =
    "generics/runtime_generic_record_instance_exit";
pub(crate) const RUNTIME_CONST_DATA_ARRAY_LENGTH_EXIT: &str =
    "generics/runtime_const_data_array_length_exit";
pub(crate) const RUNTIME_CONST_DATA_FORWARDED_LENGTH_EXIT: &str =
    "generics/runtime_const_data_forwarded_length_exit";
pub(crate) const RUNTIME_CONST_DATA_MULTIPLE_INSTANCES_EXIT: &str =
    "generics/runtime_const_data_multiple_instances_exit";
pub(crate) const RUNTIME_CONST_DATA_NAMED_VALUE_EXIT: &str =
    "generics/runtime_const_data_named_value_exit";
pub(crate) const STRUCTURED_CONST_CANONICAL_IDENTITY: &str =
    "generics/structured_const_canonical_identity";
pub(crate) const STRUCTURED_CONST_CANONICAL_RAT: &str = "generics/structured_const_canonical_rat";
pub(crate) const STRUCTURED_CONST_DEFAULT_DOMAIN_UNPROVED: &str =
    "generics/structured_const_default_domain_unproved";
pub(crate) const STRUCTURED_CONST_INELIGIBLE_FLOAT_FIELD: &str =
    "generics/structured_const_ineligible_float_field";
pub(crate) const STRUCTURED_CONST_RAT_ZERO_DENOMINATOR: &str =
    "generics/structured_const_rat_zero_denominator";
pub(crate) const STRUCTURED_CONST_RAT_UNCANCELLED: &str =
    "generics/structured_const_rat_uncancelled";
pub(crate) const STRUCTURED_CONST_RAT_UNREDUCED: &str = "generics/structured_const_rat_unreduced";
pub(crate) const CLOSED_INDEXED_QUANTITY: &str = "generics/closed_indexed_quantity";
pub(crate) const CLOSED_INDEXED_DOMAIN_MISMATCH: &str = "generics/closed_indexed_domain_mismatch";
pub(crate) const CLOSED_INDEXED_STRUCT_FIELD_MISMATCH: &str =
    "generics/closed_indexed_struct_field_mismatch";
pub(crate) const CLOSED_INDEXED_ARRAY_ELEMENT_MISMATCH: &str =
    "generics/closed_indexed_array_element_mismatch";
pub(crate) const CLOSED_INDEXED_DOMAIN_NONCANONICAL_RAT: &str =
    "generics/closed_indexed_domain_noncanonical_rat";
pub(crate) const CLOSED_INDEXED_DOMAIN_UNKNOWN_CONST: &str =
    "generics/closed_indexed_domain_unknown_const";
pub(crate) const CLOSED_INDEXED_DOMAIN_WRONG_ARITY: &str =
    "generics/closed_indexed_domain_wrong_arity";
pub(crate) const CLOSED_INDEXED_DOMAIN_WRONG_TYPE: &str =
    "generics/closed_indexed_domain_wrong_type";
pub(crate) const CLOSED_INDEXED_QUALIFICATION_UNKNOWN_CONST: &str =
    "generics/closed_indexed_qualification_unknown_const";
pub(crate) const CLOSED_INDEXED_QUALIFICATION_WRONG_ARITY: &str =
    "generics/closed_indexed_qualification_wrong_arity";
pub(crate) const CLOSED_INDEXED_QUALIFICATION_WRONG_TYPE: &str =
    "generics/closed_indexed_qualification_wrong_type";
pub(crate) const CONST_MACHINE_DESTINATION_NOT_INFERRED: &str =
    "generics/const_machine_destination_not_inferred";
pub(crate) const RUNTIME_STD_UNITS_EXIT: &str = "generics/runtime_std_units_exit";
pub(crate) const STD_UNITS_IMPLICIT_CROSS_INDEX: &str = "generics/std_units_implicit_cross_index";
pub(crate) const OPEN_COMPUTED_QUANTITY_RESULT: &str = "generics/open_computed_quantity_result";
pub(crate) const OPEN_INDEX_UNLICENSED_ALGEBRA: &str = "generics/open_index_unlicensed_algebra";
pub(crate) const OPEN_INDEX_LOCAL_FACT: &str = "generics/open_index_local_fact";
pub(crate) const OPEN_INDEX_UNESTABLISHED_EQUALITY: &str =
    "generics/open_index_unestablished_equality";
pub(crate) const RUNTIME_CONST_DATA_EXPRESSION_EXIT: &str =
    "generics/runtime_const_data_expression_exit";
pub(crate) const RUNTIME_CONST_DATA_SYMBOLIC_EXPRESSION_EXIT: &str =
    "generics/runtime_const_data_symbolic_expression_exit";
pub(crate) const RUNTIME_CONST_DATA_MACHINE_CALL_EXIT: &str =
    "generics/runtime_const_data_machine_call_exit";
pub(crate) const RUNTIME_CONST_DATA_WHERE_FACT_EXIT: &str =
    "generics/runtime_const_data_where_fact_exit";
pub(crate) const MATCH_DEFAULT_SATISFIES_EXHAUSTIVENESS: &str =
    "data/match_default_satisfies_exhaustiveness";
pub(crate) const RUNTIME_CONST_DATA_MACHINE_FACT_EXIT: &str =
    "generics/runtime_const_data_machine_fact_exit";
pub(crate) const RUNTIME_SIGNED_CONST_DATA_EXIT: &str = "generics/runtime_signed_const_data_exit";
pub(crate) const RUNTIME_TRAIT_DEFAULT_DISPATCH_EXIT: &str =
    "traits/runtime_trait_default_dispatch_exit";
pub(crate) const RUNTIME_INHERITED_TRAIT_DEFAULT_EXIT: &str =
    "traits/runtime_inherited_trait_default_exit";
pub(crate) const RUNTIME_GENERIC_TRAIT_DEFAULT_EXIT: &str =
    "traits/runtime_generic_trait_default_exit";
pub(crate) const RUNTIME_CONST_CONTAINER_METHODS_EXIT: &str =
    "generics/runtime_const_container_methods_exit";
pub(crate) const RUNTIME_GENERIC_TWO_INSTANTIATIONS_EXIT: &str =
    "generics/runtime_generic_two_instantiations_exit";
pub(crate) const RUNTIME_MIN_MAX_GUARD_SUBJECT_HOIST_EXIT: &str =
    "calls/runtime_min_max_guard_subject_hoist_exit";
pub(crate) const RUNTIME_INDEXED_GUARD_TRUE_FALSE_PAIR_EXIT: &str =
    "collections/runtime_indexed_guard_true_false_pair_exit";
pub(crate) const RUNTIME_INDEXED_FIELD_LOCAL_OPERAND_EXIT: &str =
    "collections/runtime_indexed_field_local_operand_exit";
pub(crate) const RUNTIME_INDEXED_LOCAL_BITWISE_EXIT: &str =
    "collections/runtime_indexed_local_bitwise_exit";
pub(crate) const RUNTIME_INDEXED_LOCAL_COMPARE_EXIT: &str =
    "collections/runtime_indexed_local_compare_exit";
pub(crate) const RUNTIME_MIN_GUARD_TRUE_FALSE_PAIR_EXIT: &str =
    "calls/runtime_min_guard_true_false_pair_exit";
pub(crate) const RUNTIME_NESTED_GENERIC_INSTANTIATIONS_EXIT: &str =
    "generics/runtime_nested_generic_instantiations_exit";
pub(crate) const RUNTIME_GENERIC_LET_LOCAL_INSTANTIATIONS_EXIT: &str =
    "generics/runtime_generic_let_local_instantiations_exit";
pub(crate) const RUNTIME_GENERIC_DOMAIN_INSTANTIATIONS_EXIT: &str =
    "generics/runtime_generic_domain_instantiations_exit";
pub(crate) const RUNTIME_ARRAY_MAX_AND_SUM_EXIT: &str =
    "collections/runtime_array_max_and_sum_exit";
pub(crate) const RUNTIME_INDEXED_REDUCTION_LOOP_EXIT: &str =
    "collections/runtime_indexed_reduction_loop_exit";
pub(crate) const RUNTIME_INDEXED_RMW_LOOP_EXIT: &str = "collections/runtime_indexed_rmw_loop_exit";
pub(crate) const RUNTIME_COMPUTED_INDEXED_WRITE_EXIT: &str =
    "collections/runtime_computed_indexed_write_exit";
pub(crate) const RUNTIME_NESTED_CONST_PRODUCT_INDEX_EXIT: &str =
    "collections/runtime_nested_const_product_index_exit";
pub(crate) const RUNTIME_HOISTED_INDEX_WRITE_EXIT: &str =
    "collections/runtime_hoisted_index_write_exit";
pub(crate) const RUNTIME_LET_MUT_REASSIGN_EXIT: &str = "calls/runtime_let_mut_reassign_exit";
pub(crate) const RUNTIME_TUPLE_MATRIX_EXHAUSTIVE_EXIT: &str =
    "control_flow/runtime_tuple_matrix_exhaustive_exit";
pub(crate) const RUNTIME_SUM_TUPLE_MATRIX_EXHAUSTIVE_EXIT: &str =
    "control_flow/runtime_sum_tuple_matrix_exhaustive_exit";
pub(crate) const RUNTIME_TUPLE_CASE_DESTRUCTURE_EXIT: &str =
    "control_flow/runtime_tuple_case_destructure_exit";
pub(crate) const RUNTIME_DEPENDENT_PARAM_RANGE_EXIT: &str =
    "dependent/runtime_dependent_param_range_exit";
pub(crate) const RUNTIME_DEPENDENT_PRODUCT_INDEX_EXIT: &str =
    "dependent/runtime_dependent_product_index_exit";
pub(crate) const RUNTIME_DEPENDENT_SUBTRACT_EXIT: &str =
    "dependent/runtime_dependent_subtract_exit";
pub(crate) const RUNTIME_DEPENDENT_ORDERING_CHAIN_EXIT: &str =
    "dependent/runtime_dependent_ordering_chain_exit";
pub(crate) const RUNTIME_REQUIRES_SUBTRACT_EXIT: &str = "dependent/runtime_requires_subtract_exit";
pub(crate) const RUNTIME_REQUIRES_GUARDED_CALL_EXIT: &str =
    "dependent/runtime_requires_guarded_call_exit";
pub(crate) const RUNTIME_SIBLING_LEN_INDEX_EXIT: &str = "dependent/runtime_sibling_len_index_exit";
pub(crate) const RUNTIME_BOUNDED_PRODUCT_INDEX_EXIT: &str =
    "dependent/runtime_bounded_product_index_exit";
pub(crate) const RUNTIME_DEPEND_MAPPING_EXIT: &str = "build/runtime_depend_mapping_exit";
pub(crate) const RUNTIME_CORE_ROSTER_OPS_EXIT: &str = "proofs/runtime_core_roster_ops_exit";

pub(crate) const STRUCTURED_CONST_PASS_CANARIES: &[&str] = &[
    STRUCTURED_CONST_CANONICAL_IDENTITY,
    STRUCTURED_CONST_CANONICAL_RAT,
];

pub(crate) const STRUCTURED_CONST_FAIL_CANARIES: &[(&str, &str)] = &[
    (
        STRUCTURED_CONST_DEFAULT_DOMAIN_UNPROVED,
        "default-domain facts whose index-site proof is not implemented",
    ),
    (
        STRUCTURED_CONST_INELIGIBLE_FLOAT_FIELD,
        "not eligible as a const index",
    ),
    (
        STRUCTURED_CONST_RAT_ZERO_DENOMINATOR,
        "denominator must be positive",
    ),
    (
        STRUCTURED_CONST_RAT_UNCANCELLED,
        "signed coordinates must be cancelled",
    ),
    (STRUCTURED_CONST_RAT_UNREDUCED, "must be gcd-reduced"),
];

pub(crate) const CLOSED_INDEXED_FAIL_CANARIES: &[&str] = &[
    CLOSED_INDEXED_DOMAIN_MISMATCH,
    CLOSED_INDEXED_STRUCT_FIELD_MISMATCH,
    CLOSED_INDEXED_ARRAY_ELEMENT_MISMATCH,
    CLOSED_INDEXED_DOMAIN_NONCANONICAL_RAT,
    CLOSED_INDEXED_DOMAIN_UNKNOWN_CONST,
    CLOSED_INDEXED_DOMAIN_WRONG_ARITY,
    CLOSED_INDEXED_DOMAIN_WRONG_TYPE,
    CLOSED_INDEXED_QUALIFICATION_UNKNOWN_CONST,
    CLOSED_INDEXED_QUALIFICATION_WRONG_ARITY,
    CLOSED_INDEXED_QUALIFICATION_WRONG_TYPE,
    CONST_MACHINE_DESTINATION_NOT_INFERRED,
];

pub(crate) const PASS_CANARIES: &[&str] = &[
    RUNTIME_DECREASES_U64_MEASURE_EXIT,
    RUNTIME_WRAPPING_OPERAND_TRUNCATION_EXIT,
    RUNTIME_FLOAT_COMPARE_BOOL_EXIT,
    AGGREGATE_TRANSITION_ARGS_EXIT,
    DEEP_NESTED_WRITE_PATHS_EXIT,
    ZII_DEFAULT_COMPOSITE_EXIT,
    ZII_STRING_HOST_WRITE_EXIT,
    ZII_DEFAULT_STRING_EQUALITY_EXIT,
    RUNTIME_OWNED_STRING_BYTE_VIEW_EXIT,
    EQUATABLE_SUM_STALE_PAYLOAD_EXIT,
    RUNTIME_TEXT_NOT_EQUALS_EXIT,
    RUNTIME_TEXT_EQUALS_BOOLEAN_OPERAND_EXIT,
    CASE_LITERAL_TEXTEQ_TERMINAL_EXIT,
    CASE_LITERAL_TEXTEQ_FIELD_STORE_EXIT,
    RUNTIME_TEXT_EQUALS_VALUE_POSITIONS_EXIT,
    SUM_PAYLOAD_CAST_OPERAND_FIELD_EXIT,
    RUNTIME_BRANCHING_CALLEE_CHAIN_EXIT,
    RECURSIVE_RESULT_BIND_FIRST_ARG,
    RUNTIME_RECURSIVE_RESULT_ROLES_EXIT,
    RUNTIME_TRAPPING_GUARD_OVERFLOW_TRAPS,
    RUNTIME_TRAPPING_OVERFLOW_TRAPS,
    RUNTIME_GUARD_PROVEN_COUNTER_EXIT,
    RUNTIME_GUARD_NARROWED_TRANSITION_ARG_EXIT,
    RUNTIME_GUI_WINDOW_LIFECYCLE_EXIT,
    RUNTIME_GUI_FOREGROUND_WINDOW_EXIT,
    RUNTIME_GUI_WINDOW_BLIT_EXIT,
    RUNTIME_GENERIC_VALUE_CALL_AGREEING_EXIT,
    RUNTIME_GENERIC_VALUE_CALL_EXIT,
    TRAIT_GENERIC_BOUND_STATIC_DISPATCH,
    RUNTIME_GENERIC_PARAM_POSITION_INFERENCE_EXIT,
    RUNTIME_GENERIC_MULTIPLE_SPECIALIZATIONS_EXIT,
    RUNTIME_GENERIC_ENUM_PAYLOAD_EXIT,
    RUNTIME_GENERIC_RECORD_INSTANCE_EXIT,
    RUNTIME_CONST_DATA_ARRAY_LENGTH_EXIT,
    RUNTIME_CONST_DATA_FORWARDED_LENGTH_EXIT,
    RUNTIME_CONST_DATA_MULTIPLE_INSTANCES_EXIT,
    RUNTIME_CONST_DATA_NAMED_VALUE_EXIT,
    CLOSED_INDEXED_QUANTITY,
    RUNTIME_STD_UNITS_EXIT,
    OPEN_COMPUTED_QUANTITY_RESULT,
    OPEN_INDEX_LOCAL_FACT,
    RUNTIME_CONST_DATA_EXPRESSION_EXIT,
    RUNTIME_CONST_DATA_SYMBOLIC_EXPRESSION_EXIT,
    RUNTIME_CONST_DATA_MACHINE_CALL_EXIT,
    RUNTIME_CONST_DATA_WHERE_FACT_EXIT,
    MATCH_DEFAULT_SATISFIES_EXHAUSTIVENESS,
    RUNTIME_CONST_DATA_MACHINE_FACT_EXIT,
    RUNTIME_SIGNED_CONST_DATA_EXIT,
    RUNTIME_TRAIT_DEFAULT_DISPATCH_EXIT,
    RUNTIME_INHERITED_TRAIT_DEFAULT_EXIT,
    RUNTIME_GENERIC_TRAIT_DEFAULT_EXIT,
    RUNTIME_CONST_CONTAINER_METHODS_EXIT,
    RUNTIME_GENERIC_TWO_INSTANTIATIONS_EXIT,
    RUNTIME_MIN_MAX_GUARD_SUBJECT_HOIST_EXIT,
    RUNTIME_INDEXED_GUARD_TRUE_FALSE_PAIR_EXIT,
    RUNTIME_INDEXED_FIELD_LOCAL_OPERAND_EXIT,
    RUNTIME_INDEXED_LOCAL_BITWISE_EXIT,
    RUNTIME_INDEXED_LOCAL_COMPARE_EXIT,
    RUNTIME_MIN_GUARD_TRUE_FALSE_PAIR_EXIT,
    RUNTIME_NESTED_GENERIC_INSTANTIATIONS_EXIT,
    RUNTIME_GENERIC_LET_LOCAL_INSTANTIATIONS_EXIT,
    RUNTIME_GENERIC_DOMAIN_INSTANTIATIONS_EXIT,
    RUNTIME_ARRAY_MAX_AND_SUM_EXIT,
    RUNTIME_INDEXED_REDUCTION_LOOP_EXIT,
    RUNTIME_INDEXED_RMW_LOOP_EXIT,
    RUNTIME_COMPUTED_INDEXED_WRITE_EXIT,
    RUNTIME_NESTED_CONST_PRODUCT_INDEX_EXIT,
    RUNTIME_HOISTED_INDEX_WRITE_EXIT,
    RUNTIME_LET_MUT_REASSIGN_EXIT,
    RUNTIME_TUPLE_MATRIX_EXHAUSTIVE_EXIT,
    RUNTIME_SUM_TUPLE_MATRIX_EXHAUSTIVE_EXIT,
    RUNTIME_TUPLE_CASE_DESTRUCTURE_EXIT,
    RUNTIME_DEPENDENT_PARAM_RANGE_EXIT,
    RUNTIME_DEPENDENT_PRODUCT_INDEX_EXIT,
    RUNTIME_DEPENDENT_SUBTRACT_EXIT,
    RUNTIME_DEPENDENT_ORDERING_CHAIN_EXIT,
    RUNTIME_REQUIRES_SUBTRACT_EXIT,
    RUNTIME_REQUIRES_GUARDED_CALL_EXIT,
    RUNTIME_SIBLING_LEN_INDEX_EXIT,
    RUNTIME_BOUNDED_PRODUCT_INDEX_EXIT,
    RUNTIME_DEPEND_MAPPING_EXIT,
    RUNTIME_CORE_ROSTER_OPS_EXIT,
];

pub(crate) const FILE_EXPECTATION_FAIL_CANARIES: &[&str] = &[
    STD_UNITS_IMPLICIT_CROSS_INDEX,
    OPEN_INDEX_UNLICENSED_ALGEBRA,
    OPEN_INDEX_UNESTABLISHED_EQUALITY,
];
