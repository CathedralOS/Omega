//! Corpus inputs used by the domain, control-flow, and structure tests.
//! Native exits, abnormal termination, checked facts, and targets stay with their owners.

pub(crate) const RUNTIME_COPY_THEN_READ_EXIT: &str = "arithmetic/runtime_copy_then_read_exit";
pub(crate) const RUNTIME_I64_FULL_WIDTH_EXIT: &str = "arithmetic/runtime_i64_full_width_exit";
pub(crate) const RUNTIME_CHAINED_STRING_APPEND_EXIT: &str =
    "text/runtime_chained_string_append_exit";
pub(crate) const RUNTIME_STRING_APPEND_IN_PLACE_EXIT: &str =
    "text/runtime_string_append_in_place_exit";
pub(crate) const RUNTIME_STRING_CONCAT_TWO_FIELDS_EXIT: &str =
    "text/runtime_string_concat_two_fields_exit";
pub(crate) const RUNTIME_MACHINE_STRING_APPEND_IN_PLACE_EXIT: &str =
    "text/runtime_machine_string_append_in_place_exit";
pub(crate) const RUNTIME_LOCAL_STRING_FIELD_COPY_THROUGH_MUT_EXIT: &str =
    "calls/runtime_local_string_field_copy_through_mut_exit";
pub(crate) const RUNTIME_CALL_VALUE: &str = "calls/runtime_call_value";
pub(crate) const UTF8_BOUNDARY_ESTABLISHED: &str = "text/utf8_boundary_established";
pub(crate) const NO_NUL_BOUNDARY_ESTABLISHED: &str = "text/no_nul_boundary_established";
pub(crate) const DOMAIN_FORGET_VALIDATE_TRANSITIONS: &str =
    "text/domain_forget_validate_transitions";
pub(crate) const RUNTIME_MIN_CALL_RESULT_ARITHMETIC_EXIT: &str =
    "calls/runtime_min_call_result_arithmetic_exit";
pub(crate) const RUNTIME_DIRECT_BOOLEAN_CONJUNCTION_EXIT: &str =
    "dungeon/runtime_direct_boolean_conjunction_exit";
pub(crate) const EXECUTABLE_DOMAIN_MEMBERSHIP_EXPRESSION_EXIT: &str =
    "domains/executable_domain_membership_expression_exit";
pub(crate) const EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_EXIT: &str =
    "domains/executable_imported_domain_membership_exit";
pub(crate) const EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_GUARD_EXIT: &str =
    "domains/executable_imported_domain_membership_guard_exit";
pub(crate) const EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_INTERSECTION_GUARD_EXIT: &str =
    "domains/executable_imported_domain_membership_intersection_guard_exit";
pub(crate) const EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_UNION_GUARD_EXIT: &str =
    "domains/executable_imported_domain_membership_union_guard_exit";
pub(crate) const EXECUTABLE_DOMAIN_MEMBERSHIP_INTERSECTION_GUARD_EXIT: &str =
    "domains/executable_domain_membership_intersection_guard_exit";
pub(crate) const EXECUTABLE_DOMAIN_MEMBERSHIP_UNION_GUARD_EXIT: &str =
    "domains/executable_domain_membership_union_guard_exit";
pub(crate) const EXECUTABLE_DOMAIN_MEMBERSHIP_UNION_VALUE_EXIT: &str =
    "domains/executable_domain_membership_union_value_exit";
pub(crate) const EXECUTABLE_DOMAIN_MEMBERSHIP_INTERSECTION_VALUE_EXIT: &str =
    "domains/executable_domain_membership_intersection_value_exit";
pub(crate) const EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_UNION_VALUE_EXIT: &str =
    "domains/executable_imported_domain_membership_union_value_exit";
pub(crate) const EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_INTERSECTION_VALUE_EXIT: &str =
    "domains/executable_imported_domain_membership_intersection_value_exit";
pub(crate) const RUNTIME_LOCAL_BOOLEAN_OR_VALUE_EXIT: &str =
    "control_flow/runtime_local_boolean_or_value_exit";
pub(crate) const RUNTIME_STRAIGHT_LINE_TERMINAL_LOCAL_EXIT: &str =
    "control_flow/runtime_straight_line_terminal_local_exit";
pub(crate) const RUNTIME_STRAIGHT_LINE_TERMINAL_FIELD_READBACK_EXIT: &str =
    "control_flow/runtime_straight_line_terminal_field_readback_exit";
pub(crate) const GUARDED_TRANSITION_DISPATCH: &str = "control_flow/guarded_transition_dispatch";
pub(crate) const RECORD_ARRAY_FIELD_ACCESS: &str = "collections/record_array_field_access";
pub(crate) const RUNTIME_NEGATED_BOOLEAN_PLACE_GUARD_EXIT: &str =
    "control_flow/runtime_negated_boolean_place_guard_exit";
pub(crate) const RUNTIME_LOCAL_BOOLEAN_CONJUNCTION_VALUE_EXIT: &str =
    "control_flow/runtime_local_boolean_conjunction_value_exit";
pub(crate) const RUNTIME_LOCAL_SCALAR_COMPARISON_VALUE_EXIT: &str =
    "control_flow/runtime_local_scalar_comparison_value_exit";
pub(crate) const RUNTIME_LOCAL_STRING_COMPARISON_VALUE_EXIT: &str =
    "control_flow/runtime_local_string_comparison_value_exit";
pub(crate) const RUNTIME_BOOLEAN_OR_GUARD_EXIT: &str = "control_flow/runtime_boolean_or_guard_exit";
pub(crate) const RUNTIME_DIRECT_BOOLEAN_TRANSITION_ARGUMENT_EXIT: &str =
    "control_flow/runtime_direct_boolean_transition_argument_exit";
pub(crate) const RUNTIME_LOCAL_BOOLEAN_TRANSITION_ARGUMENT_EXIT: &str =
    "control_flow/runtime_local_boolean_transition_argument_exit";
pub(crate) const RUNTIME_BOOLEAN_TRANSITION_ARGUMENT_AFTER_STRING_GUARD_EXIT: &str =
    "control_flow/runtime_boolean_transition_argument_after_string_guard_exit";
pub(crate) const RUNTIME_MACHINE_OWNED_INDEXED_NESTED_ROOM_COPY_EXIT: &str =
    "storage/runtime_machine_owned_indexed_nested_room_copy_exit";
pub(crate) const RUNTIME_NEGATED_COMPARISON_GUARD_EXIT: &str =
    "control_flow/runtime_negated_comparison_guard_exit";
pub(crate) const RUNTIME_CASE_MEMBER_DISPATCH_EXIT: &str =
    "control_flow/runtime_case_member_dispatch_exit";
pub(crate) const CASE_PAYLOAD_NATIVE_CONSTRUCTION: &str = "data/case_payload_native_construction";
pub(crate) const RUNTIME_RECORD_FIELD_VALUE_PATTERN_EXIT: &str =
    "data/runtime_record_field_value_pattern_exit";
pub(crate) const RUNTIME_CASE_PAYLOAD_GUARD_READ_EXIT: &str =
    "data/runtime_case_payload_guard_read_exit";
pub(crate) const CASE_MEMBERSHIP_VALUE_EXIT: &str = "data/case_membership_value_exit";
pub(crate) const MATCH_EXHAUSTIVE_BY_CASES: &str = "data/match_exhaustive_by_cases";
pub(crate) const MATCH_EXHAUSTIVE_BY_CASE_UNION_DOMAIN: &str =
    "data/match_exhaustive_by_case_union_domain";
pub(crate) const CASE_MEMBERSHIP_UNION_GUARD_EXIT: &str = "data/case_membership_union_guard_exit";
pub(crate) const RUNTIME_CASE_REASSIGNMENT_EXIT: &str = "data/runtime_case_reassignment_exit";
pub(crate) const RUNTIME_MIXED_SHAPE_EXIT: &str = "data/runtime_mixed_shape_exit";
pub(crate) const RUNTIME_ARRAY_LITERAL_STRING_FIELD_EXIT: &str =
    "data/runtime_array_literal_string_field_exit";
pub(crate) const RUNTIME_STRUCT_LITERAL_STRING_FIELD_EXIT: &str =
    "data/runtime_struct_literal_string_field_exit";
pub(crate) const RUNTIME_PARAM_DOMAIN_FORWARD_EXIT: &str = "text/runtime_param_domain_forward_exit";
pub(crate) const RUNTIME_CASE_PAYLOAD_DOMAIN_FORWARD_EXIT: &str =
    "text/runtime_case_payload_domain_forward_exit";
pub(crate) const RUNTIME_TUPLE_TRANSITION_EXIT: &str = "control_flow/runtime_tuple_transition_exit";
pub(crate) const RUNTIME_ROOM_USE_REENTRY_EXIT: &str = "dungeon/runtime_room_use_reentry_exit";
pub(crate) const RUNTIME_ENEMY_CLEAR_REENTRY_EXIT: &str =
    "dungeon/runtime_enemy_clear_reentry_exit";
pub(crate) const RUNTIME_CLEAR_CARVE_RENDER_STRING_FIELDS_EXIT: &str =
    "dungeon/runtime_clear_carve_render_string_fields_exit";
pub(crate) const RUNTIME_FULL_LEVEL_WRAPPER_LOOKUP_STRING_FIELD_EXIT: &str =
    "dungeon/runtime_full_level_wrapper_lookup_string_field_exit";
pub(crate) const RUNTIME_MULTI_ROOM_REENTRY_EXIT: &str = "dungeon/runtime_multi_room_reentry_exit";
pub(crate) const RUNTIME_MUTABLE_SLICE_ELEMENT_WRITE_EXIT: &str =
    "slices/runtime_mutable_slice_element_write_exit";
pub(crate) const RUNTIME_MUTABLE_SLICE_ELEMENT_WRITE_STRAIGHT_LINE_EXIT: &str =
    "slices/runtime_mutable_slice_element_write_straight_line_exit";
pub(crate) const RUNTIME_DISPATCH_MUTABLE_SLICE_ELEMENT_WRITE_EXIT: &str =
    "slices/runtime_dispatch_mutable_slice_element_write_exit";
pub(crate) const RUNTIME_ARRAY_INDEXED_READ_EXIT: &str = "slices/runtime_array_indexed_read_exit";
pub(crate) const RUNTIME_INDEXED_STRUCT_FIELD_WRITE_EXIT: &str =
    "slices/runtime_indexed_struct_field_write_exit";
pub(crate) const RUNTIME_PARTICLE_SYSTEM_EXIT: &str = "structs/runtime_particle_system_exit";
pub(crate) const RUNTIME_NESTED_STRUCT_CONSTRUCTION_EXIT: &str =
    "structs/runtime_nested_struct_construction_exit";
pub(crate) const RUNTIME_CROSS_MACHINE_SUBSTATE_NAME_EXIT: &str =
    "calls/runtime_cross_machine_substate_name_exit";
pub(crate) const RUNTIME_VALUE_CALL_TO_ARRAY_ELEMENT_EXIT: &str =
    "calls/runtime_value_call_to_array_element_exit";
pub(crate) const RUNTIME_COMPUTED_TRANSITION_ARGS_EXIT: &str =
    "calls/runtime_computed_transition_args_exit";
pub(crate) const RUNTIME_STRUCT_BY_VALUE_PARAM_EXIT: &str =
    "calls/runtime_struct_by_value_param_exit";
pub(crate) const RUNTIME_VALUE_CALL_COMPOSITION_EXIT: &str =
    "calls/runtime_value_call_composition_exit";
pub(crate) const RUNTIME_STRUCT_VALUE_CALL_EXIT: &str = "calls/runtime_struct_value_call_exit";
pub(crate) const RUNTIME_OPTION_VALUE_CALL_EXIT: &str = "calls/runtime_option_value_call_exit";
pub(crate) const RUNTIME_RESULT_MATCH_EXIT: &str = "errors/runtime_result_match_exit";
pub(crate) const RUNTIME_ENTITY_COMPONENT_EXIT: &str = "structs/runtime_entity_component_exit";
pub(crate) const RUNTIME_NESTED_STRUCT_STATE_MACHINE_EXIT: &str =
    "structs/runtime_nested_struct_state_machine_exit";
pub(crate) const RUNTIME_ARRAY_ELEMENT_STRUCT_COPY_EXIT: &str =
    "structs/runtime_array_element_struct_copy_exit";
pub(crate) const RUNTIME_NESTED_STRUCT_VALUE_SEMANTICS_EXIT: &str =
    "structs/runtime_nested_struct_value_semantics_exit";
pub(crate) const RUNTIME_STRUCT_ARRAY_LITERAL_EXIT: &str =
    "structs/runtime_struct_array_literal_exit";
pub(crate) const RUNTIME_ENUM_STRUCT_PAYLOAD_EXIT: &str =
    "structs/runtime_enum_struct_payload_exit";
pub(crate) const RUNTIME_ENUM_CLASSIFY_DISPATCH_EXIT: &str =
    "structs/runtime_enum_classify_dispatch_exit";
pub(crate) const RUNTIME_NESTED_FIELD_ACCUMULATE_LOOP_EXIT: &str =
    "structs/runtime_nested_field_accumulate_loop_exit";
pub(crate) const RUNTIME_INDEXED_WRITE_CONST_READ_EXIT: &str =
    "slices/runtime_indexed_write_const_read_exit";
pub(crate) const RUNTIME_INDEXED_RMW_TEMP_EXIT: &str = "slices/runtime_indexed_rmw_temp_exit";
pub(crate) const RUNTIME_INDEXED_WRITE_ADJACENT_FIELD_EXIT: &str =
    "slices/runtime_indexed_write_adjacent_field_exit";
pub(crate) const RUNTIME_JOIN_MEET_BOUND_EXIT: &str = "slices/runtime_join_meet_bound_exit";
pub(crate) const RUNTIME_DUAL_INDEXED_COMPARISON_GUARD_EXIT: &str =
    "collections/runtime_dual_indexed_comparison_guard_exit";
pub(crate) const RUNTIME_ARRAY_MIN_MAX_BUILTIN_EXIT: &str =
    "collections/runtime_array_min_max_builtin_exit";
pub(crate) const RUNTIME_INDEXED_GUARD_SUBJECT_EXIT: &str =
    "collections/runtime_indexed_guard_subject_exit";
pub(crate) const RUNTIME_TICK_PACED_MARQUEE_EXIT: &str = "host/runtime_tick_paced_marquee_exit";
pub(crate) const RUNTIME_USER32_KEY_STATE_EXIT: &str = "host/runtime_user32_key_state_exit";
pub(crate) const RUNTIME_TICK_COUNT_MONOTONIC_EXIT: &str = "host/runtime_tick_count_monotonic_exit";
pub(crate) const RUNTIME_GUI_MEMORY_DC_BLIT_EXIT: &str = "host/runtime_gui_memory_dc_blit_exit";
pub(crate) const RUNTIME_NESTED_PAYLOAD_RANGE_NARROWING_EXIT: &str =
    "arithmetic/runtime_nested_payload_range_narrowing_exit";
pub(crate) const RUNTIME_INLINE_RECURSIVE_WALK_EXIT: &str =
    "calls/runtime_inline_recursive_walk_exit";
pub(crate) const RUNTIME_VALUE_CALL_DIRECT_RECURSIVE_WALK_EXIT: &str =
    "calls/runtime_value_call_direct_recursive_walk_exit";
pub(crate) const RUNTIME_VALUE_CALL_STATEMENT_RECURSIVE_WALK_EXIT: &str =
    "calls/runtime_value_call_statement_recursive_walk_exit";
pub(crate) const RUNTIME_SATURATING_WIDE_BOUNDARIES_EXIT: &str =
    "arithmetic/runtime_saturating_wide_boundaries_exit";
pub(crate) const RUNTIME_SATURATING_PARAM_CARRY_EXIT: &str =
    "arithmetic/runtime_saturating_param_carry_exit";
pub(crate) const RUNTIME_SATURATING_EXPRESSION_DOMAIN_EXIT: &str =
    "arithmetic/runtime_saturating_expression_domain_exit";
pub(crate) const RUNTIME_WRAPPING_EXPRESSION_GUARD_EXIT: &str =
    "arithmetic/runtime_wrapping_expression_guard_exit";
pub(crate) const RUNTIME_DIVIDE_MIN_EDGE_GUARD_EXIT: &str =
    "arithmetic/runtime_divide_min_edge_guard_exit";
pub(crate) const RUNTIME_NESTED_UNSIGNED_WITNESS_EXIT: &str =
    "arithmetic/runtime_nested_unsigned_witness_exit";
pub(crate) const RUNTIME_LOCAL_ARRAY_ELEMENT_VALUE_OPERAND_EXIT: &str =
    "slices/runtime_local_array_element_value_operand_exit";
pub(crate) const RUNTIME_MACHINE_ARRAY_ELEMENT_FUSED_CALL_ARG_EXIT: &str =
    "slices/runtime_machine_array_element_fused_call_arg_exit";
pub(crate) const RUNTIME_SATURATING_ARRAY_ELEMENT_GUARD_EXIT: &str =
    "slices/runtime_saturating_array_element_guard_exit";
pub(crate) const CUSTOM_RANKING_FIELD_COUNTDOWN_COMPILE: &str =
    "termination/custom_ranking_field_countdown_compile";
pub(crate) const CUSTOM_RANKING_STRUCT_VIEW: &str = "termination/custom_ranking_struct_view";
pub(crate) const RUNTIME_FLOAT_NESTED_OPERAND_EXIT: &str =
    "arithmetic/runtime_float_nested_operand_exit";
pub(crate) const RUNTIME_SHIFT_COUNT_DOMAIN_EXIT: &str =
    "arithmetic/runtime_shift_count_domain_exit";
pub(crate) const RUNTIME_EXACT_GUARDED_SHIFT_COUNT_EXIT: &str =
    "arithmetic/runtime_exact_guarded_shift_count_exit";
pub(crate) const RUNTIME_SHIFT_ATWIDTH_SIGNED_MODULAR_EXIT: &str =
    "arithmetic/runtime_shift_atwidth_signed_modular_exit";
pub(crate) const RUNTIME_SHIFT_RIGHT_ATWIDTH_EXIT: &str =
    "arithmetic/runtime_shift_right_atwidth_exit";
pub(crate) const RUNTIME_SHIFT_ATWIDTH_INDEXED_TARGETS_EXIT: &str =
    "arithmetic/runtime_shift_atwidth_indexed_targets_exit";
pub(crate) const RUNTIME_SAT_NESTED_OPERAND_DOMAIN_EXIT: &str =
    "arithmetic/runtime_sat_nested_operand_domain_exit";
pub(crate) const RUNTIME_SAT_UNSIGNED_ONEDIRECTION_EXIT: &str =
    "arithmetic/runtime_sat_unsigned_onedirection_exit";
pub(crate) const RUNTIME_SAT_MIN_IDIOM_EXIT: &str = "arithmetic/runtime_sat_min_idiom_exit";
pub(crate) const RUNTIME_SHL_SATURATING_EXIT: &str = "arithmetic/runtime_shl_saturating_exit";
pub(crate) const RUNTIME_SHL_SATURATING_VALUE_OVERFLOW_EXIT: &str =
    "arithmetic/runtime_shl_saturating_value_overflow_exit";
pub(crate) const RUNTIME_SHIFT_SUBWORD_MASKED_COUNT_EXIT: &str =
    "arithmetic/runtime_shift_subword_masked_count_exit";
pub(crate) const FLOAT_TO_INT_SATURATING_EXIT: &str = "arithmetic/float_to_int_saturating_exit";
pub(crate) const FLOAT_TO_INT_UNSIGNED_NARROW_SATURATING_EXIT: &str =
    "arithmetic/float_to_int_unsigned_narrow_saturating_exit";
pub(crate) const FLOAT_SATURATING_OVERFLOW_EXIT: &str = "arithmetic/float_saturating_overflow_exit";
pub(crate) const FLOAT_TRAPPING_OVERFLOW_TRAPS: &str = "arithmetic/float_trapping_overflow_traps";
pub(crate) const FLOAT_TRAPPING_DIVZERO_TRAPS: &str = "arithmetic/float_trapping_divzero_traps";
pub(crate) const FLOAT_TRAPPING_INVALID_TRAPS: &str = "arithmetic/float_trapping_invalid_traps";
pub(crate) const TRAPPING_FLOAT_TO_INT_CAST_TRAPS: &str =
    "arithmetic/trapping_float_to_int_cast_traps";
pub(crate) const TRAPPING_FLOAT_TO_NARROW_INT_CAST_TRAPS: &str =
    "arithmetic/trapping_float_to_narrow_int_cast_traps";
pub(crate) const TRAPPING_SHIFT_COUNT_TRAPS: &str = "arithmetic/trapping_shift_count_traps";
pub(crate) const FLOAT_LITERAL_CAST_PROVES_EXIT: &str = "arithmetic/float_literal_cast_proves_exit";
pub(crate) const U64_MAGNITUDE_TRANSITION_ARG_EXIT: &str =
    "arithmetic/u64_magnitude_transition_arg_exit";
pub(crate) const RUNTIME_SHIFT_COUNT_PROVEN_RANGE_EXIT: &str =
    "arithmetic/runtime_shift_count_proven_range_exit";

pub(crate) const BOUNDARY_DOMAIN_ESTABLISHMENT_PASS_CANARIES: &[&str] = &[
    UTF8_BOUNDARY_ESTABLISHED,
    NO_NUL_BOUNDARY_ESTABLISHED,
    DOMAIN_FORGET_VALIDATE_TRANSITIONS,
];

pub(crate) const ROOTED_RESIDUAL_SCALAR_ENTRY_PASS_CANARIES: &[(&str, i32)] = &[
    (GUARDED_TRANSITION_DISPATCH, 0),
    (RECORD_ARRAY_FIELD_ACCESS, 0),
];

pub(crate) const RECURSIVE_WALK_PASS_CANARIES: &[&str] = &[
    RUNTIME_INLINE_RECURSIVE_WALK_EXIT,
    RUNTIME_VALUE_CALL_DIRECT_RECURSIVE_WALK_EXIT,
    RUNTIME_VALUE_CALL_STATEMENT_RECURSIVE_WALK_EXIT,
];

pub(crate) const PASS_CANARIES: &[&str] = &[
    RUNTIME_COPY_THEN_READ_EXIT,
    RUNTIME_I64_FULL_WIDTH_EXIT,
    RUNTIME_CHAINED_STRING_APPEND_EXIT,
    RUNTIME_STRING_APPEND_IN_PLACE_EXIT,
    RUNTIME_STRING_CONCAT_TWO_FIELDS_EXIT,
    RUNTIME_MACHINE_STRING_APPEND_IN_PLACE_EXIT,
    RUNTIME_LOCAL_STRING_FIELD_COPY_THROUGH_MUT_EXIT,
    RUNTIME_CALL_VALUE,
    RUNTIME_MIN_CALL_RESULT_ARITHMETIC_EXIT,
    RUNTIME_DIRECT_BOOLEAN_CONJUNCTION_EXIT,
    EXECUTABLE_DOMAIN_MEMBERSHIP_EXPRESSION_EXIT,
    EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_EXIT,
    EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_GUARD_EXIT,
    EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_INTERSECTION_GUARD_EXIT,
    EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_UNION_GUARD_EXIT,
    EXECUTABLE_DOMAIN_MEMBERSHIP_INTERSECTION_GUARD_EXIT,
    EXECUTABLE_DOMAIN_MEMBERSHIP_UNION_GUARD_EXIT,
    EXECUTABLE_DOMAIN_MEMBERSHIP_UNION_VALUE_EXIT,
    EXECUTABLE_DOMAIN_MEMBERSHIP_INTERSECTION_VALUE_EXIT,
    EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_UNION_VALUE_EXIT,
    EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_INTERSECTION_VALUE_EXIT,
    RUNTIME_LOCAL_BOOLEAN_OR_VALUE_EXIT,
    RUNTIME_STRAIGHT_LINE_TERMINAL_LOCAL_EXIT,
    RUNTIME_STRAIGHT_LINE_TERMINAL_FIELD_READBACK_EXIT,
    RUNTIME_NEGATED_BOOLEAN_PLACE_GUARD_EXIT,
    RUNTIME_LOCAL_BOOLEAN_CONJUNCTION_VALUE_EXIT,
    RUNTIME_LOCAL_SCALAR_COMPARISON_VALUE_EXIT,
    RUNTIME_LOCAL_STRING_COMPARISON_VALUE_EXIT,
    RUNTIME_BOOLEAN_OR_GUARD_EXIT,
    RUNTIME_DIRECT_BOOLEAN_TRANSITION_ARGUMENT_EXIT,
    RUNTIME_LOCAL_BOOLEAN_TRANSITION_ARGUMENT_EXIT,
    RUNTIME_BOOLEAN_TRANSITION_ARGUMENT_AFTER_STRING_GUARD_EXIT,
    RUNTIME_MACHINE_OWNED_INDEXED_NESTED_ROOM_COPY_EXIT,
    RUNTIME_NEGATED_COMPARISON_GUARD_EXIT,
    RUNTIME_CASE_MEMBER_DISPATCH_EXIT,
    CASE_PAYLOAD_NATIVE_CONSTRUCTION,
    RUNTIME_RECORD_FIELD_VALUE_PATTERN_EXIT,
    RUNTIME_CASE_PAYLOAD_GUARD_READ_EXIT,
    CASE_MEMBERSHIP_VALUE_EXIT,
    MATCH_EXHAUSTIVE_BY_CASES,
    MATCH_EXHAUSTIVE_BY_CASE_UNION_DOMAIN,
    CASE_MEMBERSHIP_UNION_GUARD_EXIT,
    RUNTIME_CASE_REASSIGNMENT_EXIT,
    RUNTIME_MIXED_SHAPE_EXIT,
    RUNTIME_ARRAY_LITERAL_STRING_FIELD_EXIT,
    RUNTIME_STRUCT_LITERAL_STRING_FIELD_EXIT,
    RUNTIME_PARAM_DOMAIN_FORWARD_EXIT,
    RUNTIME_CASE_PAYLOAD_DOMAIN_FORWARD_EXIT,
    RUNTIME_TUPLE_TRANSITION_EXIT,
    RUNTIME_ROOM_USE_REENTRY_EXIT,
    RUNTIME_ENEMY_CLEAR_REENTRY_EXIT,
    RUNTIME_CLEAR_CARVE_RENDER_STRING_FIELDS_EXIT,
    RUNTIME_FULL_LEVEL_WRAPPER_LOOKUP_STRING_FIELD_EXIT,
    RUNTIME_MULTI_ROOM_REENTRY_EXIT,
    RUNTIME_MUTABLE_SLICE_ELEMENT_WRITE_EXIT,
    RUNTIME_MUTABLE_SLICE_ELEMENT_WRITE_STRAIGHT_LINE_EXIT,
    RUNTIME_DISPATCH_MUTABLE_SLICE_ELEMENT_WRITE_EXIT,
    RUNTIME_ARRAY_INDEXED_READ_EXIT,
    RUNTIME_INDEXED_STRUCT_FIELD_WRITE_EXIT,
    RUNTIME_PARTICLE_SYSTEM_EXIT,
    RUNTIME_NESTED_STRUCT_CONSTRUCTION_EXIT,
    RUNTIME_CROSS_MACHINE_SUBSTATE_NAME_EXIT,
    RUNTIME_VALUE_CALL_TO_ARRAY_ELEMENT_EXIT,
    RUNTIME_COMPUTED_TRANSITION_ARGS_EXIT,
    RUNTIME_STRUCT_BY_VALUE_PARAM_EXIT,
    RUNTIME_VALUE_CALL_COMPOSITION_EXIT,
    RUNTIME_STRUCT_VALUE_CALL_EXIT,
    RUNTIME_OPTION_VALUE_CALL_EXIT,
    RUNTIME_RESULT_MATCH_EXIT,
    RUNTIME_ENTITY_COMPONENT_EXIT,
    RUNTIME_NESTED_STRUCT_STATE_MACHINE_EXIT,
    RUNTIME_ARRAY_ELEMENT_STRUCT_COPY_EXIT,
    RUNTIME_NESTED_STRUCT_VALUE_SEMANTICS_EXIT,
    RUNTIME_STRUCT_ARRAY_LITERAL_EXIT,
    RUNTIME_ENUM_STRUCT_PAYLOAD_EXIT,
    RUNTIME_ENUM_CLASSIFY_DISPATCH_EXIT,
    RUNTIME_NESTED_FIELD_ACCUMULATE_LOOP_EXIT,
    RUNTIME_INDEXED_WRITE_CONST_READ_EXIT,
    RUNTIME_INDEXED_RMW_TEMP_EXIT,
    RUNTIME_INDEXED_WRITE_ADJACENT_FIELD_EXIT,
    RUNTIME_JOIN_MEET_BOUND_EXIT,
    RUNTIME_DUAL_INDEXED_COMPARISON_GUARD_EXIT,
    RUNTIME_ARRAY_MIN_MAX_BUILTIN_EXIT,
    RUNTIME_INDEXED_GUARD_SUBJECT_EXIT,
    RUNTIME_TICK_PACED_MARQUEE_EXIT,
    RUNTIME_USER32_KEY_STATE_EXIT,
    RUNTIME_TICK_COUNT_MONOTONIC_EXIT,
    RUNTIME_GUI_MEMORY_DC_BLIT_EXIT,
    RUNTIME_NESTED_PAYLOAD_RANGE_NARROWING_EXIT,
    RUNTIME_SATURATING_WIDE_BOUNDARIES_EXIT,
    RUNTIME_SATURATING_PARAM_CARRY_EXIT,
    RUNTIME_SATURATING_EXPRESSION_DOMAIN_EXIT,
    RUNTIME_WRAPPING_EXPRESSION_GUARD_EXIT,
    RUNTIME_DIVIDE_MIN_EDGE_GUARD_EXIT,
    RUNTIME_NESTED_UNSIGNED_WITNESS_EXIT,
    RUNTIME_LOCAL_ARRAY_ELEMENT_VALUE_OPERAND_EXIT,
    RUNTIME_MACHINE_ARRAY_ELEMENT_FUSED_CALL_ARG_EXIT,
    RUNTIME_SATURATING_ARRAY_ELEMENT_GUARD_EXIT,
    CUSTOM_RANKING_FIELD_COUNTDOWN_COMPILE,
    CUSTOM_RANKING_STRUCT_VIEW,
    RUNTIME_FLOAT_NESTED_OPERAND_EXIT,
    RUNTIME_SHIFT_COUNT_DOMAIN_EXIT,
    RUNTIME_EXACT_GUARDED_SHIFT_COUNT_EXIT,
    RUNTIME_SHIFT_ATWIDTH_SIGNED_MODULAR_EXIT,
    RUNTIME_SHIFT_RIGHT_ATWIDTH_EXIT,
    RUNTIME_SHIFT_ATWIDTH_INDEXED_TARGETS_EXIT,
    RUNTIME_SAT_NESTED_OPERAND_DOMAIN_EXIT,
    RUNTIME_SAT_UNSIGNED_ONEDIRECTION_EXIT,
    RUNTIME_SAT_MIN_IDIOM_EXIT,
    RUNTIME_SHL_SATURATING_EXIT,
    RUNTIME_SHL_SATURATING_VALUE_OVERFLOW_EXIT,
    RUNTIME_SHIFT_SUBWORD_MASKED_COUNT_EXIT,
    FLOAT_TO_INT_SATURATING_EXIT,
    FLOAT_TO_INT_UNSIGNED_NARROW_SATURATING_EXIT,
    FLOAT_SATURATING_OVERFLOW_EXIT,
    FLOAT_TRAPPING_OVERFLOW_TRAPS,
    TRAPPING_FLOAT_TO_INT_CAST_TRAPS,
    TRAPPING_FLOAT_TO_NARROW_INT_CAST_TRAPS,
    TRAPPING_SHIFT_COUNT_TRAPS,
    FLOAT_LITERAL_CAST_PROVES_EXIT,
    U64_MAGNITUDE_TRANSITION_ARG_EXIT,
    RUNTIME_SHIFT_COUNT_PROVEN_RANGE_EXIT,
    FLOAT_TRAPPING_DIVZERO_TRAPS,
    FLOAT_TRAPPING_INVALID_TRAPS,
];
