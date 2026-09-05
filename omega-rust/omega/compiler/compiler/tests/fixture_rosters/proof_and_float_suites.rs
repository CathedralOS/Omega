//! Fixture identities and execution tables shared by proof/float tests and inventory.

pub const WIRE_DECODE_REQUIREMENT_SURFACE: &str = "wire/decode_requirement_surface";
pub const DEPENDENT_DATA_WHERE_STANDING_BOUND_EXIT: &str =
    "dependent/data_where_standing_bound_exit";
pub const DEPENDENT_DATA_WHERE_STANDING_BOUND_ABSENT_REJECTED: &str =
    "dependent/data_where_standing_bound_absent_rejected";
pub const DEPENDENT_DATA_WHERE_PRODUCT_HYPOTHESIS: &str = "dependent/data_where_product_hypothesis";
pub const PROOFS_NAT_EXACT_SUBTRACTION_COMPILE: &str = "proofs/nat_exact_subtraction_compile";
pub const PROOFS_NAT_EXACT_SUBTRACTION_REQUIRES_ORDER: &str =
    "proofs/nat_exact_subtraction_requires_order";
pub const PROOFS_RING_REARRANGE_CORE_NAT: &str = "proofs/ring_rearrange_core_nat";
pub const PROOFS_RING_IDENTITY_SLOT_BRIDGE_COMPILE: &str =
    "proofs/ring_identity_slot_bridge_compile";
pub const PROOFS_INTEGER_MEASURED_NAT_INDUCTION_COMPILE: &str =
    "proofs/integer_measured_nat_induction_compile";
pub const TERMINATION_PROOF_NON_TAIL_JOINT_MACHINE_CYCLE_COMPILE: &str =
    "termination/proof_non_tail_joint_machine_cycle_compile";
pub const FLOAT_FLOAT_TO_INT_EXACT_PROOFS_EXIT: &str = "float/float_to_int_exact_proofs_exit";
pub const ARITHMETIC_RUNTIME_FLOAT_MIN_MAX_ABS_CLAMP_EXIT: &str =
    "arithmetic/runtime_float_min_max_abs_clamp_exit";
pub const CALLS_RUNTIME_SHARED_REF_PARAM_MEMBER_EXIT: &str =
    "calls/runtime_shared_ref_param_member_exit";
pub const CALLS_RUNTIME_SHARED_REF_PARAM_LARGE_DEREF_EXIT: &str =
    "calls/runtime_shared_ref_param_large_deref_exit";
pub const CALLS_RUNTIME_LARGE_SHARED_REF_DIRECT_ASSIGNMENT_EXIT: &str =
    "calls/runtime_large_shared_ref_direct_assignment_exit";
pub const CALLS_RUNTIME_SAME_TYPE_CONTAINED_DIRECT_FIELDS_EXIT: &str =
    "calls/runtime_same_type_contained_direct_fields_exit";
pub const CONTROL_FLOW_RUNTIME_SUM_FIELD_STORE_PAYLOAD_EXIT: &str =
    "control_flow/runtime_sum_field_store_payload_exit";
pub const COLLECTIONS_RUNTIME_ARGMAX_INDEX_EXIT: &str = "collections/runtime_argmax_index_exit";
pub const COLLECTIONS_RUNTIME_BRACKET_MATCHER_STACK_EXIT: &str =
    "collections/runtime_bracket_matcher_stack_exit";
pub const COLLECTIONS_RUNTIME_PALINDROME_TWO_POINTER_EXIT: &str =
    "collections/runtime_palindrome_two_pointer_exit";
pub const COLLECTIONS_RUNTIME_CROSS_ARRAY_INDEXED_GUARD_COMPARE_EXIT: &str =
    "collections/runtime_cross_array_indexed_guard_compare_exit";
pub const COLLECTIONS_RUNTIME_DUAL_INDEXED_GUARD_EQUALITY_EXIT: &str =
    "collections/runtime_dual_indexed_guard_equality_exit";
pub const COLLECTIONS_RUNTIME_DUAL_INDEXED_GUARD_COMPARE_EXIT: &str =
    "collections/runtime_dual_indexed_guard_compare_exit";
pub const ARITHMETIC_RUNTIME_FLOAT_RUNNING_MIN_MAX_FOLD_EXIT: &str =
    "arithmetic/runtime_float_running_min_max_fold_exit";
pub const ARITHMETIC_RUNTIME_CLAMP_DESUGAR_EXIT: &str = "arithmetic/runtime_clamp_desugar_exit";
pub const ARITHMETIC_RUNTIME_CLAMP_NARROWING_EXIT: &str = "arithmetic/runtime_clamp_narrowing_exit";
pub const ARITHMETIC_RUNTIME_NEGATIVE_FLOAT_TO_INT_EXIT: &str =
    "arithmetic/runtime_negative_float_to_int_exit";
pub const FLOAT_FLOAT_TO_INT_POLICY_EXIT: &str = "float/float_to_int_policy_exit";
pub const FLOAT_FLOAT_SATURATING_ARITHMETIC_EXIT: &str = "float/float_saturating_arithmetic_exit";
pub const ARITHMETIC_RUNTIME_SQRT_BUILTIN_EXIT: &str = "arithmetic/runtime_sqrt_builtin_exit";
pub const ARITHMETIC_RUNTIME_ABS_DESUGAR_EXIT: &str = "arithmetic/runtime_abs_desugar_exit";
pub const ARITHMETIC_RUNTIME_FLOAT_SELF_COMPARE_NAN_EXIT: &str =
    "arithmetic/runtime_float_self_compare_nan_exit";
pub const FLOAT_RUNTIME_TOTAL_ORDER_SATISFIERS_EXIT: &str =
    "float/runtime_total_order_satisfiers_exit";
pub const FLOAT_BUILD_RUNTIME_SEMANTICS_TWINS: &str = "float/build_runtime_semantics_twins";
pub const FLOAT_BUILD_RUNTIME_SEMANTICS_TWINS_X86_BASELINE: &str =
    "float/build_runtime_semantics_twins_x86_baseline";
pub const FLOAT_BUILD_RUNTIME_SEMANTICS_TWINS_WINDOWS_X64: &str =
    "float/build_runtime_semantics_twins_windows_x64";
pub const DEPENDENT_RANGE_SUGAR_GATED_CONSTRUCTION_COMPILE: &str =
    "dependent/range_sugar_gated_construction_compile";
pub const DEPENDENT_NESTED_GATED_CONSTRUCTION_COMPILE: &str =
    "dependent/nested_gated_construction_compile";
pub const DEPENDENT_ZERO_CASE_ABSORBS_NESTED_GATE_COMPILE: &str =
    "dependent/zero_case_absorbs_nested_gate_compile";
pub const DEPENDENT_RANGE_GATED_MACHINE_ESTABLISHMENT_COMPILE: &str =
    "dependent/range_gated_machine_establishment_compile";
pub const DEPENDENT_DATA_WHERE_CROSS_STATE_ESTABLISH: &str =
    "dependent/data_where_cross_state_establish";
pub const DEPENDENT_DATA_WHERE_CALLEE_ESTABLISHES: &str = "dependent/data_where_callee_establishes";
pub const DEPENDENT_DATA_WHERE_MULTISTATE_CALLEE: &str = "dependent/data_where_multistate_callee";
pub const DEPENDENT_DATA_WHERE_GATED_LITERAL_PROVES: &str =
    "dependent/data_where_gated_literal_proves";
pub const ARITHMETIC_ZII_RANGE_EXCLUDES_ZERO_REJECTED: &str =
    "arithmetic/zii_range_excludes_zero_rejected";
pub const RANGE_ELEMENT_RANGE_ZERO_EXCLUDED: &str = "range/element_range_zero_excluded";
pub const DEPENDENT_RANGE_SUGAR_GATED_FIELD_OMITTED_REJECTED: &str =
    "dependent/range_sugar_gated_field_omitted_rejected";
pub const DEPENDENT_NESTED_GATED_FIELD_OMITTED_REJECTED: &str =
    "dependent/nested_gated_field_omitted_rejected";
pub const DEPENDENT_DATA_WHERE_GATED_MACHINE_UNESTABLISHED_REJECTED: &str =
    "dependent/data_where_gated_machine_unestablished_rejected";
pub const DEPENDENT_DATA_WHERE_MEMBERSHIP_LITERAL_COMPILE: &str =
    "dependent/data_where_membership_literal_compile";
pub const DEPENDENT_DATA_WHERE_MEMBERSHIP_WINDOW_RESTORED_COMPILE: &str =
    "dependent/data_where_membership_window_restored_compile";
pub const DEPENDENT_DATA_WHERE_MEMBERSHIP_ZERO_VALID_COMPILE: &str =
    "dependent/data_where_membership_zero_valid_compile";
pub const DEPENDENT_DATA_WHERE_MEMBERSHIP_LITERAL_REJECTED: &str =
    "dependent/data_where_membership_literal_rejected";
pub const DEPENDENT_DATA_WHERE_MEMBERSHIP_CARRIER_MISMATCH_REJECTED: &str =
    "dependent/data_where_membership_carrier_mismatch_rejected";
pub const DEPENDENT_DATA_WHERE_AMBIGUOUS_DOMAIN_SHORT_NAME_REJECTED: &str =
    "dependent/data_where_ambiguous_domain_short_name_rejected";
pub const DEPENDENT_DATA_WHERE_LENGTH_CONSTRUCTION_COMPILE: &str =
    "dependent/data_where_length_construction_compile";
pub const DEPENDENT_DATA_WHERE_LENGTH_WINDOW_COMPILE: &str =
    "dependent/data_where_length_window_compile";
pub const DEPENDENT_DATA_WHERE_LENGTH_ZERO_VALID_COMPILE: &str =
    "dependent/data_where_length_zero_valid_compile";
pub const DEPENDENT_DATA_WHERE_SYMBOLIC_EQUAL_CONSTRUCTION_COMPILE: &str =
    "dependent/data_where_symbolic_equal_construction_compile";
pub const DEPENDENT_DATA_WHERE_SYMBOLIC_EQUAL_WINDOW_COMPILE: &str =
    "dependent/data_where_symbolic_equal_window_compile";
pub const DEPENDENT_DATA_WHERE_CAPACITY_MEASURE_COMPILE: &str =
    "dependent/data_where_capacity_measure_compile";
pub const DEPENDENT_DATA_WHERE_LENGTH_MISMATCH_REJECTED: &str =
    "dependent/data_where_length_mismatch_rejected";
pub const DEPENDENT_DATA_WHERE_CAPACITY_MISMATCH_REJECTED: &str =
    "dependent/data_where_capacity_mismatch_rejected";
pub const DEPENDENT_DATA_WHERE_PARAM_WRITE_UNPROVEN: &str =
    "dependent/data_where_param_write_unproven";
pub const DEPENDENT_DATA_WHERE_CROSS_STATE_UNKNOWN_REFUSES: &str =
    "dependent/data_where_cross_state_unknown_refuses";
pub const DEPENDENT_DATA_WHERE_SYMBOLIC_CORRELATION_STALE_REJECTED: &str =
    "dependent/data_where_symbolic_correlation_stale_rejected";
pub const DEPENDENT_DATA_WHERE_INVARIANT_WINDOW_UNCLOSED_REJECTED: &str =
    "dependent/data_where_invariant_window_unclosed_rejected";
pub const DEPENDENT_DATA_WHERE_READ_BEFORE_ESTABLISH: &str =
    "dependent/data_where_read_before_establish";
pub const DEPENDENT_DATA_WHERE_SYMBOLIC_AFFINE_WINDOW_COMPILE: &str =
    "dependent/data_where_symbolic_affine_window_compile";
pub const DEPENDENT_DATA_WHERE_COMMUTATIVE_CORRELATION_COMPILE: &str =
    "dependent/data_where_commutative_correlation_compile";
pub const DEPENDENT_DATA_WHERE_FLOW_PROVEN_CONSTRUCTION_COMPILE: &str =
    "dependent/data_where_flow_proven_construction_compile";
pub const TRAITS_RING_REQUIREMENT_SATISFIES_EXIT: &str = "traits/ring_requirement_satisfies_exit";
pub const PROOFS_POLYNOMIAL_EXPAND_CORE_NAT: &str = "proofs/polynomial_expand_core_nat";
pub const PROOFS_PROOF_NAT_STRUCTURAL_LEMMAS: &str = "proofs/proof_nat_structural_lemmas";
pub const PROOFS_RING_REARRANGE_UNLICENSED_REJECTED: &str =
    "proofs/ring_rearrange_unlicensed_rejected";
pub const PROOFS_RING_REARRANGE_FALSE_SHUFFLE_REJECTED: &str =
    "proofs/ring_rearrange_false_shuffle_rejected";
pub const TERMINATION_PROOF_JOINT_MACHINE_CYCLE_NONDECREASING: &str =
    "termination/proof_joint_machine_cycle_nondecreasing";
pub const TERMINATION_PROOF_JOINT_MACHINE_CYCLE_UNMEASURED: &str =
    "termination/proof_joint_machine_cycle_unmeasured";
pub const ARITHMETIC_FLOAT_CAST_UNPROVEN_REJECTED: &str = "arithmetic/float_cast_unproven_rejected";
pub const ARITHMETIC_FLOAT_TO_INT_EXACT_UNPROVEN: &str = "arithmetic/float_to_int_exact_unproven";
pub const FLOAT_FLOAT_TO_INT_TRAPPING_NAN_TRAPS: &str = "float/float_to_int_trapping_nan_traps";
pub const FLOAT_FLOAT_TO_INT_TRAPPING_OVERFLOW_TRAPS: &str =
    "float/float_to_int_trapping_overflow_traps";
pub const FLOAT_FLOAT_TRAPPING_OVERFLOW_TRAPS: &str = "float/float_trapping_overflow_traps";
pub const FLOAT_FLOAT_TRAPPING_DIVIDE_ZERO_TRAPS: &str = "float/float_trapping_divide_zero_traps";
pub const FLOAT_FLOAT_TRAPPING_INVALID_TRAPS: &str = "float/float_trapping_invalid_traps";
pub const FLOAT_FLOAT_TRAPPING_PROPAGATED_NAN_TRAPS: &str =
    "float/float_trapping_propagated_nan_traps";
pub const FLOAT_FLOAT_TRAPPING_PROPAGATED_INFINITY_TRAPS: &str =
    "float/float_trapping_propagated_infinity_traps";

pub const PASS_CANARIES: &[&str] = &[
    WIRE_DECODE_REQUIREMENT_SURFACE,
    DEPENDENT_DATA_WHERE_STANDING_BOUND_EXIT,
    DEPENDENT_DATA_WHERE_PRODUCT_HYPOTHESIS,
    PROOFS_NAT_EXACT_SUBTRACTION_COMPILE,
    PROOFS_RING_IDENTITY_SLOT_BRIDGE_COMPILE,
    PROOFS_INTEGER_MEASURED_NAT_INDUCTION_COMPILE,
    TERMINATION_PROOF_NON_TAIL_JOINT_MACHINE_CYCLE_COMPILE,
    FLOAT_FLOAT_TO_INT_EXACT_PROOFS_EXIT,
    ARITHMETIC_RUNTIME_FLOAT_MIN_MAX_ABS_CLAMP_EXIT,
    CALLS_RUNTIME_SHARED_REF_PARAM_MEMBER_EXIT,
    CALLS_RUNTIME_SHARED_REF_PARAM_LARGE_DEREF_EXIT,
    CALLS_RUNTIME_LARGE_SHARED_REF_DIRECT_ASSIGNMENT_EXIT,
    CALLS_RUNTIME_SAME_TYPE_CONTAINED_DIRECT_FIELDS_EXIT,
    CONTROL_FLOW_RUNTIME_SUM_FIELD_STORE_PAYLOAD_EXIT,
    COLLECTIONS_RUNTIME_ARGMAX_INDEX_EXIT,
    COLLECTIONS_RUNTIME_BRACKET_MATCHER_STACK_EXIT,
    COLLECTIONS_RUNTIME_PALINDROME_TWO_POINTER_EXIT,
    COLLECTIONS_RUNTIME_CROSS_ARRAY_INDEXED_GUARD_COMPARE_EXIT,
    COLLECTIONS_RUNTIME_DUAL_INDEXED_GUARD_EQUALITY_EXIT,
    COLLECTIONS_RUNTIME_DUAL_INDEXED_GUARD_COMPARE_EXIT,
    ARITHMETIC_RUNTIME_FLOAT_RUNNING_MIN_MAX_FOLD_EXIT,
    ARITHMETIC_RUNTIME_CLAMP_DESUGAR_EXIT,
    ARITHMETIC_RUNTIME_CLAMP_NARROWING_EXIT,
    ARITHMETIC_RUNTIME_NEGATIVE_FLOAT_TO_INT_EXIT,
    FLOAT_FLOAT_TO_INT_POLICY_EXIT,
    FLOAT_FLOAT_SATURATING_ARITHMETIC_EXIT,
    ARITHMETIC_RUNTIME_SQRT_BUILTIN_EXIT,
    ARITHMETIC_RUNTIME_ABS_DESUGAR_EXIT,
    ARITHMETIC_RUNTIME_FLOAT_SELF_COMPARE_NAN_EXIT,
    FLOAT_RUNTIME_TOTAL_ORDER_SATISFIERS_EXIT,
    FLOAT_BUILD_RUNTIME_SEMANTICS_TWINS,
    FLOAT_BUILD_RUNTIME_SEMANTICS_TWINS_X86_BASELINE,
    FLOAT_BUILD_RUNTIME_SEMANTICS_TWINS_WINDOWS_X64,
];

pub const FILE_EXPECTATION_FAIL_CANARIES: &[&str] =
    &[DEPENDENT_DATA_WHERE_STANDING_BOUND_ABSENT_REJECTED];

pub const FAIL_CANARIES: &[&str] = &[PROOFS_NAT_EXACT_SUBTRACTION_REQUIRES_ORDER];

pub const RANGE_GATED_ESTABLISHMENT_PASS_CANARIES: &[&str] = &[
    DEPENDENT_RANGE_SUGAR_GATED_CONSTRUCTION_COMPILE,
    DEPENDENT_NESTED_GATED_CONSTRUCTION_COMPILE,
    DEPENDENT_ZERO_CASE_ABSORBS_NESTED_GATE_COMPILE,
    DEPENDENT_RANGE_GATED_MACHINE_ESTABLISHMENT_COMPILE,
    DEPENDENT_DATA_WHERE_CROSS_STATE_ESTABLISH,
    DEPENDENT_DATA_WHERE_CALLEE_ESTABLISHES,
    DEPENDENT_DATA_WHERE_MULTISTATE_CALLEE,
    DEPENDENT_DATA_WHERE_GATED_LITERAL_PROVES,
];

pub const RANGE_GATED_ESTABLISHMENT_FILE_FAIL_CANARIES: &[&str] = &[
    ARITHMETIC_ZII_RANGE_EXCLUDES_ZERO_REJECTED,
    RANGE_ELEMENT_RANGE_ZERO_EXCLUDED,
    DEPENDENT_RANGE_SUGAR_GATED_FIELD_OMITTED_REJECTED,
    DEPENDENT_NESTED_GATED_FIELD_OMITTED_REJECTED,
    DEPENDENT_DATA_WHERE_GATED_MACHINE_UNESTABLISHED_REJECTED,
];

pub const DEFAULT_DOMAIN_MEMBERSHIP_PASS_CANARIES: &[&str] = &[
    DEPENDENT_DATA_WHERE_MEMBERSHIP_LITERAL_COMPILE,
    DEPENDENT_DATA_WHERE_MEMBERSHIP_WINDOW_RESTORED_COMPILE,
    DEPENDENT_DATA_WHERE_MEMBERSHIP_ZERO_VALID_COMPILE,
];

pub const DEFAULT_DOMAIN_MEMBERSHIP_FILE_FAIL_CANARIES: &[&str] = &[
    DEPENDENT_DATA_WHERE_MEMBERSHIP_LITERAL_REJECTED,
    DEPENDENT_DATA_WHERE_MEMBERSHIP_CARRIER_MISMATCH_REJECTED,
    DEPENDENT_DATA_WHERE_AMBIGUOUS_DOMAIN_SHORT_NAME_REJECTED,
];

pub const DEFAULT_DOMAIN_MEASURE_PASS_CANARIES: &[&str] = &[
    DEPENDENT_DATA_WHERE_LENGTH_CONSTRUCTION_COMPILE,
    DEPENDENT_DATA_WHERE_LENGTH_WINDOW_COMPILE,
    DEPENDENT_DATA_WHERE_LENGTH_ZERO_VALID_COMPILE,
    DEPENDENT_DATA_WHERE_SYMBOLIC_EQUAL_CONSTRUCTION_COMPILE,
    DEPENDENT_DATA_WHERE_SYMBOLIC_EQUAL_WINDOW_COMPILE,
    DEPENDENT_DATA_WHERE_CAPACITY_MEASURE_COMPILE,
];

pub const DEFAULT_DOMAIN_MEASURE_FILE_FAIL_CANARIES: &[&str] = &[
    DEPENDENT_DATA_WHERE_LENGTH_MISMATCH_REJECTED,
    DEPENDENT_DATA_WHERE_CAPACITY_MISMATCH_REJECTED,
];

pub const DEFAULT_DOMAIN_STALE_FACT_FAIL_CANARIES: &[&str] = &[
    DEPENDENT_DATA_WHERE_PARAM_WRITE_UNPROVEN,
    DEPENDENT_DATA_WHERE_CROSS_STATE_UNKNOWN_REFUSES,
    DEPENDENT_DATA_WHERE_SYMBOLIC_CORRELATION_STALE_REJECTED,
    DEPENDENT_DATA_WHERE_INVARIANT_WINDOW_UNCLOSED_REJECTED,
];

pub const DEFAULT_DOMAIN_PRODUCT_FAIL_CANARIES: &[&str] = &[
    DEPENDENT_DATA_WHERE_GATED_MACHINE_UNESTABLISHED_REJECTED,
    DEPENDENT_DATA_WHERE_READ_BEFORE_ESTABLISH,
    DEPENDENT_DATA_WHERE_INVARIANT_WINDOW_UNCLOSED_REJECTED,
];

pub const DEFAULT_DOMAIN_CORRELATION_PASS_CANARIES: &[&str] = &[
    DEPENDENT_DATA_WHERE_SYMBOLIC_AFFINE_WINDOW_COMPILE,
    DEPENDENT_DATA_WHERE_COMMUTATIVE_CORRELATION_COMPILE,
    DEPENDENT_DATA_WHERE_FLOW_PROVEN_CONSTRUCTION_COMPILE,
];

pub const DEFAULT_DOMAIN_CORRELATION_FAIL_CANARIES: &[&str] = &[
    DEPENDENT_DATA_WHERE_SYMBOLIC_CORRELATION_STALE_REJECTED,
    DEPENDENT_DATA_WHERE_CROSS_STATE_UNKNOWN_REFUSES,
];

pub const COMMUTATIVE_SEMIRING_PASS_CANARIES: &[&str] = &[
    PROOFS_RING_REARRANGE_CORE_NAT,
    TRAITS_RING_REQUIREMENT_SATISFIES_EXIT,
];

pub const COMMUTATIVE_SEMIRING_CHECKED_PASS_CANARIES: &[&str] = &[
    PROOFS_POLYNOMIAL_EXPAND_CORE_NAT,
    PROOFS_PROOF_NAT_STRUCTURAL_LEMMAS,
];

pub const ALGEBRAIC_NORMALIZATION_FAIL_CANARIES: &[&str] = &[
    PROOFS_RING_REARRANGE_UNLICENSED_REJECTED,
    PROOFS_RING_REARRANGE_FALSE_SHUFFLE_REJECTED,
];

pub const PROOF_JOINT_RANKING_FAIL_CANARIES: &[(&str, &str)] = &[
    (
        TERMINATION_PROOF_JOINT_MACHINE_CYCLE_NONDECREASING,
        "does not structurally decrease",
    ),
    (
        TERMINATION_PROOF_JOINT_MACHINE_CYCLE_UNMEASURED,
        "unmeasured proof machine",
    ),
];

pub const EXACT_FLOAT_TO_INT_FAIL_CANARIES: &[&str] = &[
    ARITHMETIC_FLOAT_CAST_UNPROVEN_REJECTED,
    ARITHMETIC_FLOAT_TO_INT_EXACT_UNPROVEN,
];

pub const FLOAT_TO_INT_TRAPPING_PASS_CANARIES: &[&str] = &[
    FLOAT_FLOAT_TO_INT_TRAPPING_NAN_TRAPS,
    FLOAT_FLOAT_TO_INT_TRAPPING_OVERFLOW_TRAPS,
];

pub const FLOAT_TRAPPING_ARITHMETIC_PASS_CANARIES: &[&str] = &[
    FLOAT_FLOAT_TRAPPING_OVERFLOW_TRAPS,
    FLOAT_FLOAT_TRAPPING_DIVIDE_ZERO_TRAPS,
    FLOAT_FLOAT_TRAPPING_INVALID_TRAPS,
    FLOAT_FLOAT_TRAPPING_PROPAGATED_NAN_TRAPS,
    FLOAT_FLOAT_TRAPPING_PROPAGATED_INFINITY_TRAPS,
];
