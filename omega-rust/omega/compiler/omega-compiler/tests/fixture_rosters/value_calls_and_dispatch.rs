//! Corpus inputs used by the value-call and dynamic-dispatch tests.
//! Target selection, custody checks, and native assertions stay with their owners.

pub(crate) const RUNTIME_INDEXED_COPY_AGGREGATE_HANDOFF_EXIT: &str =
    "calls/runtime_indexed_copy_aggregate_handoff_exit";
pub(crate) const RUNTIME_MUTABLE_CALL_BEFORE_TRANSITION_ARGS_EXIT: &str =
    "calls/runtime_mutable_call_before_transition_args_exit";
pub(crate) const RUNTIME_REFERENCED_LOCAL_OUTLIVES_SIBLING_GUARD_CALL_EXIT: &str =
    "calls/runtime_referenced_local_outlives_sibling_guard_call_exit";
pub(crate) const RUNTIME_VIEW_LINKED_INPUT_UNRELATED_REF_WRITE_EXIT: &str =
    "borrow/runtime_view_linked_input_unrelated_ref_write_exit";
pub(crate) const RUNTIME_VALUE_CALL_SINGLE_EXECUTION_EXIT: &str =
    "calls/runtime_value_call_single_execution_exit";
pub(crate) const RUNTIME_EXPLICIT_DISCARD_EXECUTES_EXIT: &str =
    "calls/runtime_explicit_discard_executes_exit";
pub(crate) const RUNTIME_TRANSITION_SUBJECT_CALL_SINGLE_EVALUATION_EXIT: &str =
    "calls/runtime_transition_subject_call_single_evaluation_exit";
pub(crate) const RUNTIME_NONPLACE_RECORD_PATTERN_SINGLE_EVALUATION_EXIT: &str =
    "control_flow/runtime_nonplace_record_pattern_single_evaluation_exit";
pub(crate) const RUNTIME_EFFECTFUL_SUBJECT_SINGLE_EVALUATION_EXIT: &str =
    "control_flow/runtime_effectful_subject_single_evaluation_exit";
pub(crate) const RUNTIME_STATEMENT_CALL_SINGLE_EXECUTION_EXIT: &str =
    "control_flow/runtime_statement_call_single_execution_exit";
pub(crate) const RUNTIME_ASSIGNMENT_CALL_POST_MUTATION_VALUE_EXIT: &str =
    "calls/runtime_assignment_call_post_mutation_value_exit";
pub(crate) const RUNTIME_VALUE_CALL_RETURN_TYPES_EXIT: &str =
    "calls/runtime_value_call_return_types_exit";
pub(crate) const RUNTIME_VALUE_CALL_STRUCT_RESULT_TO_TARGET_EXIT: &str =
    "calls/runtime_value_call_struct_result_to_target_exit";
pub(crate) const RUNTIME_VALUE_CALL_SELF_FIELD_ENUM_MATCH_EXIT: &str =
    "calls/runtime_value_call_self_field_enum_match_exit";
pub(crate) const RUNTIME_VALUE_CALL_STRUCT_LITERAL_ARMS_EXIT: &str =
    "calls/runtime_value_call_struct_literal_arms_exit";
pub(crate) const RUNTIME_CONTAINED_MACHINE_EXIT: &str = "calls/runtime_contained_machine_exit";
pub(crate) const RUNTIME_CALL_RESULT_AFTER_SPLICE_MUTATION_EXIT: &str =
    "calls/runtime_call_result_after_splice_mutation_exit";
pub(crate) const RUNTIME_CALLED_MACHINE_LOOP_SEARCH_EXIT: &str =
    "calls/runtime_called_machine_loop_search_exit";
pub(crate) const RUNTIME_TRAILING_LOCAL_RETURN_EXIT: &str =
    "calls/runtime_trailing_local_return_exit";
pub(crate) const RUNTIME_LOOPING_VALUE_RETURN_EXIT: &str =
    "calls/runtime_looping_value_return_exit";
pub(crate) const RUNTIME_LOOPING_CAST_RETURN_EXIT: &str = "calls/runtime_looping_cast_return_exit";
pub(crate) const RUNTIME_VALUE_CALL_SLICE_LEN_GUARD_EXIT: &str =
    "calls/runtime_value_call_slice_len_guard_exit";
pub(crate) const RUNTIME_SLEEP_EXIT: &str = "host/runtime_sleep_exit";
pub(crate) const RUNTIME_WRITE_NO_NEWLINE_EXIT: &str = "host/runtime_write_no_newline_exit";
pub(crate) const RUNTIME_EXIT_CODE_EXIT: &str = "calls/runtime_exit_code_exit";
pub(crate) const BORROW_CARRYING_DATA_FIELD_EXIT: &str =
    "expressions/borrow_carrying_data_field_exit";
pub(crate) const RUNTIME_U8_FIELD_ARITH_EXIT: &str = "types/runtime_u8_field_arith_exit";
pub(crate) const RUNTIME_I8_SIGNED_ARITH_EXIT: &str = "types/runtime_i8_signed_arith_exit";
pub(crate) const RUNTIME_I16_SIGNED_ARITH_EXIT: &str = "types/runtime_i16_signed_arith_exit";
pub(crate) const RUNTIME_U16_FIELD_ARITH_EXIT: &str = "types/runtime_u16_field_arith_exit";
pub(crate) const RUNTIME_ADDR_FIELD_EXIT: &str = "types/runtime_addr_field_exit";
pub(crate) const RUNTIME_I64_SIGNED_ARITH_EXIT: &str = "types/runtime_i64_signed_arith_exit";
pub(crate) const RUNTIME_ADDR_VALUE_FLOW_EXIT: &str = "types/runtime_addr_value_flow_exit";
pub(crate) const RUNTIME_ADDR_ALGEBRA_EXIT: &str = "types/runtime_addr_algebra_exit";
pub(crate) const RUNTIME_REF_PARAM_METHOD_DISPATCH_EXIT: &str =
    "traits/runtime_ref_param_method_dispatch_exit";
pub(crate) const RUNTIME_TYPED_TWO_METHOD_RECEIVERS_EXIT: &str =
    "traits/runtime_typed_two_method_receivers_exit";
pub(crate) const RUNTIME_DYN_SINGLE_IMPL_DISPATCH_EXIT: &str =
    "traits/runtime_dyn_single_impl_dispatch_exit";
pub(crate) const RUNTIME_LOCAL_NAMED_DYN_DEVIRTUALIZED_EXIT: &str =
    "traits/runtime_local_named_dyn_devirtualized_exit";
pub(crate) const RUNTIME_LOCAL_NAMED_DYN_PASS_THROUGH_EXIT: &str =
    "traits/runtime_local_named_dyn_pass_through_exit";
pub(crate) const RUNTIME_LOCAL_NAMED_DYN_UNIT_MULTI_HOP_RETURN: &str =
    "traits/runtime_local_named_dyn_unit_multi_hop_return";
pub(crate) const RUNTIME_LOCAL_NAMED_DYN_REBOUND_DIRECT_EXIT: &str =
    "traits/runtime_local_named_dyn_rebound_direct_exit";
pub(crate) const RUNTIME_LOCAL_NAMED_DYN_STORED_RETURN: &str =
    "traits/runtime_local_named_dyn_stored_return";
pub(crate) const RUNTIME_LOCAL_NAMED_DYN_STORED_EXIT: &str =
    "traits/runtime_local_named_dyn_stored_exit";
pub(crate) const RUNTIME_DYN_TWO_IMPL_DISPATCH_EXIT: &str =
    "traits/runtime_dyn_two_impl_dispatch_exit";
pub(crate) const RUNTIME_DYN_TWO_IMPL_DISPATCH_SWAPPED_EXIT: &str =
    "traits/runtime_dyn_two_impl_dispatch_swapped_exit";
pub(crate) const RUNTIME_ALIAS_WRITE_THROUGH_GUARDED_TRANSITION_EXIT: &str =
    "calls/runtime_alias_write_through_guarded_transition_exit";
pub(crate) const RUNTIME_REFERENCE_PARAM_FORWARDED_THROUGH_LOOP_EXIT: &str =
    "calls/runtime_reference_param_forwarded_through_loop_exit";
pub(crate) const RUNTIME_VALUE_CALL_THROUGH_ALIAS_IN_DISPATCH_EXIT: &str =
    "calls/runtime_value_call_through_alias_in_dispatch_exit";
pub(crate) const RUNTIME_NESTED_VALUE_CALL_IN_SUBSTATE_EXIT: &str =
    "calls/runtime_nested_value_call_in_substate_exit";
pub(crate) const RUNTIME_CALL_IN_INLINED_SUBSTATE_EXIT: &str =
    "calls/runtime_call_in_inlined_substate_exit";
pub(crate) const RUNTIME_ALIAS_INDEXED_READ_THROUGH_TRANSITION_EXIT: &str =
    "calls/runtime_alias_indexed_read_through_transition_exit";
pub(crate) const RUNTIME_DISPATCH_BINARY_CALL_ARGUMENT_EXIT: &str =
    "calls/runtime_dispatch_binary_call_argument_exit";
pub(crate) const RUNTIME_DISPATCH_RESULT_FIELD_BINDING_EXIT: &str =
    "calls/runtime_dispatch_result_field_binding_exit";
pub(crate) const RUNTIME_TRAILING_STATE_MUT_PARAM_PHASE_EXIT: &str =
    "calls/runtime_trailing_state_mut_param_phase_exit";
pub(crate) const RUNTIME_SAME_TYPE_SECOND_RECEIVER_MUTATION_EXIT: &str =
    "calls/runtime_same_type_second_receiver_mutation_exit";
pub(crate) const RUNTIME_DISPATCH_FLOAT_TERMINAL_EXIT: &str =
    "calls/runtime_dispatch_float_terminal_exit";
pub(crate) const RUNTIME_VALUE_MACHINE_RECEIVER_FIELD_POSTENTRY_EXIT: &str =
    "time/runtime_value_machine_receiver_field_postentry_exit";
pub(crate) const RUNTIME_NESTED_RECEIVER_SAME_TYPE_EXIT: &str =
    "references/runtime_nested_receiver_same_type_exit";
pub(crate) const RUNTIME_DISPATCH_SECOND_RECEIVER_EXIT: &str =
    "calls/runtime_dispatch_second_receiver_exit";
pub(crate) const RUNTIME_DISPATCH_SIBLING_VALUE_CALLS_EXIT: &str =
    "calls/runtime_dispatch_sibling_value_calls_exit";
pub(crate) const RUNTIME_INLINE_REPEATED_RECEIVER_VALUE_CALLS_EXIT: &str =
    "calls/runtime_inline_repeated_receiver_value_calls_exit";
pub(crate) const RUNTIME_NONENTRY_SECOND_RECEIVER_EXIT: &str =
    "calls/runtime_nonentry_second_receiver_exit";
pub(crate) const RUNTIME_SELFCALL_CHAIN_SECOND_RECEIVER_EXIT: &str =
    "calls/runtime_selfcall_chain_second_receiver_exit";
pub(crate) const RUNTIME_NESTED_INLINE_CHAIN_RESULT_EXIT: &str =
    "calls/runtime_nested_inline_chain_result_exit";
pub(crate) const RUNTIME_NONENTRY_INLINE_SECOND_RECEIVER_EXIT: &str =
    "calls/runtime_nonentry_inline_second_receiver_exit";
pub(crate) const RUNTIME_NESTED_LOCAL_TERMINAL_SECOND_INSTANCE_EXIT: &str =
    "calls/runtime_nested_local_terminal_second_instance_exit";
pub(crate) const RUNTIME_NESTED_FIELD_TERMINAL_SECOND_INSTANCE_EXIT: &str =
    "calls/runtime_nested_field_terminal_second_instance_exit";
pub(crate) const RUNTIME_MULTIARM_SAME_NAMED_LOCALS_EXIT: &str =
    "calls/runtime_multiarm_same_named_locals_exit";
pub(crate) const RUNTIME_MULTIARM_TEXTEQ_LOCAL_EXIT: &str =
    "calls/runtime_multiarm_texteq_local_exit";
pub(crate) const RUNTIME_PRE_GUARD_TEXTEQ_LOCAL_GUARD_EXIT: &str =
    "calls/runtime_pre_guard_texteq_local_guard_exit";
pub(crate) const RUNTIME_PRE_GUARD_TEXTEQ_LOCAL_ARG_FORWARD_EXIT: &str =
    "calls/runtime_pre_guard_texteq_local_arg_forward_exit";
pub(crate) const RUNTIME_PARAM_RECEIVER_SECOND_INSTANCE_EXIT: &str =
    "calls/runtime_param_receiver_second_instance_exit";
pub(crate) const RUNTIME_PARAM_FORWARD_CHAIN_SECOND_RECEIVER_EXIT: &str =
    "calls/runtime_param_forward_chain_second_receiver_exit";
pub(crate) const RUNTIME_MAIN_SOURCE_BUILDER_IS_ORDINARY_EXIT: &str =
    "build/runtime_main_source_builder_is_ordinary_exit";
pub(crate) const RUNTIME_SATURATING_TIME_ARITH_EXIT: &str =
    "time/runtime_saturating_time_arith_exit";
pub(crate) const RUNTIME_NATURAL_TERMINATION_EXIT: &str = "core/runtime_natural_termination_exit";
pub(crate) const RUNTIME_DEEP_STATE_NAME_COLLISION_EXIT: &str =
    "calls/runtime_deep_state_name_collision_exit";
pub(crate) const RUNTIME_U64_LITERAL_LET_GUARD_EXIT: &str =
    "arithmetic/runtime_u64_literal_let_guard_exit";
pub(crate) const RUNTIME_PARAM_RECEIVER_SINGLE_INSTANCE_EXIT: &str =
    "calls/runtime_param_receiver_single_instance_exit";
pub(crate) const RUNTIME_DISPATCH_RESULT_ALIAS_READ_EXIT: &str =
    "calls/runtime_dispatch_result_alias_read_exit";
pub(crate) const RUNTIME_DISPATCH_SLICE_ELEMENT_TERMINAL_EXIT: &str =
    "calls/runtime_dispatch_slice_element_terminal_exit";
pub(crate) const RUNTIME_DISPATCH_RESULT_BINARY_TERMINAL_EXIT: &str =
    "calls/runtime_dispatch_result_binary_terminal_exit";
pub(crate) const RUNTIME_DISPATCH_RESULT_MULTI_ARM_EXIT: &str =
    "calls/runtime_dispatch_result_multi_arm_exit";
pub(crate) const RUNTIME_DISPATCH_RESULT_GUARD_SUBJECT_EXIT: &str =
    "calls/runtime_dispatch_result_guard_subject_exit";
pub(crate) const RUNTIME_DISPATCH_RESULT_TRANSITION_ARG_EXIT: &str =
    "calls/runtime_dispatch_result_transition_arg_exit";
pub(crate) const RUNTIME_DISPATCHED_EFFECTFUL_REENTRANT_EXIT: &str =
    "calls/runtime_dispatched_effectful_reentrant_exit";
pub(crate) const RUNTIME_DISPATCH_RESULT_ENUM_CASE_EXIT: &str =
    "calls/runtime_dispatch_result_enum_case_exit";
pub(crate) const RUNTIME_DISPATCH_MACHINE_ARRAY_SLICE_ARG_EXIT: &str =
    "calls/runtime_dispatch_machine_array_slice_arg_exit";
pub(crate) const RUNTIME_DISPATCH_RESULT_FIELD_TERMINAL_EXIT: &str =
    "calls/runtime_dispatch_result_field_terminal_exit";
pub(crate) const RUNTIME_NESTED_CALLED_MACHINE_LOOP_EXIT: &str =
    "calls/runtime_nested_called_machine_loop_exit";
pub(crate) const RUNTIME_STATE_LOOP_INDEXED_SEARCH_EXIT: &str =
    "control_flow/runtime_state_loop_indexed_search_exit";
pub(crate) const RUNTIME_CALL_RESULT_THROUGH_REFERENCE_FIELD_EXIT: &str =
    "calls/runtime_call_result_through_reference_field_exit";
pub(crate) const RUNTIME_STRING_CALL_RESULT_THROUGH_REFERENCE_FIELD_EXIT: &str =
    "calls/runtime_string_call_result_through_reference_field_exit";
pub(crate) const RUNTIME_TWO_STRING_CALL_RESULTS_THROUGH_REFERENCE_FIELDS_EXIT: &str =
    "calls/runtime_two_string_call_results_through_reference_fields_exit";
pub(crate) const RUNTIME_OFFSET_STRING_CALL_RESULTS_THROUGH_REFERENCE_FIELDS_EXIT: &str =
    "calls/runtime_offset_string_call_results_through_reference_fields_exit";
pub(crate) const RUNTIME_REFERENCE_RETURNED_SLICE_ELEMENT_WRITE_EXIT: &str =
    "calls/runtime_reference_returned_slice_element_write_exit";
pub(crate) const RUNTIME_REFERENCE_RETURNED_SLICE_ELEMENT_THROUGH_PARAM_EXIT: &str =
    "calls/runtime_reference_returned_slice_element_through_param_exit";
pub(crate) const RUNTIME_NESTED_GUARDED_REFERENCE_RETURNED_SLICE_ELEMENT_EXIT: &str =
    "calls/runtime_nested_guarded_reference_returned_slice_element_exit";
pub(crate) const RUNTIME_MUTABLE_LOCAL_INDEXED_PARAMETER_WRITE_EXIT: &str =
    "calls/runtime_mutable_local_indexed_parameter_write_exit";
pub(crate) const RUNTIME_MUTABLE_MACHINE_OWNED_LOCAL_INDEXED_PARAMETER_WRITE_EXIT: &str =
    "calls/runtime_mutable_machine_owned_local_indexed_parameter_write_exit";
pub(crate) const RUNTIME_MUTABLE_DYNAMIC_INDEXED_MACHINE_OWNED_PARAMETER_WRITE_EXIT: &str =
    "calls/runtime_mutable_dynamic_indexed_machine_owned_parameter_write_exit";
pub(crate) const RUNTIME_DISPATCH_LOCAL_INDEX_BINARY_WRITE_EXIT: &str =
    "storage/runtime_dispatch_local_index_binary_write_exit";
pub(crate) const RUNTIME_DISPATCH_HELPER_LOCAL_ALIAS_ADD_EXIT: &str =
    "storage/runtime_dispatch_helper_local_alias_add_exit";
pub(crate) const RUNTIME_SLICE_ALIAS_INDEXED_FIELD_WRITE_EXIT: &str =
    "storage/runtime_slice_alias_indexed_field_write_exit";
pub(crate) const RUNTIME_SLICE_INDEXED_BINARY_RMW_EXIT: &str =
    "storage/runtime_slice_indexed_binary_rmw_exit";
pub(crate) const RUNTIME_MUT_REF_FORWARD_EXIT: &str = "calls/runtime_mut_ref_forward_exit";
pub(crate) const RUNTIME_LOCAL_SLICE_FORWARD_EXIT: &str =
    "storage/runtime_local_slice_forward_exit";
pub(crate) const F32_GUARD_CONST_ARITH_LANDED_EXIT: &str =
    "float/f32_guard_const_arith_landed_exit";
pub(crate) const F32_ARG_CONST_ARITH_LANDED_EXIT: &str = "float/f32_arg_const_arith_landed_exit";
pub(crate) const RUNTIME_LOCAL_NAMED_DYN_MUTABLE_PASS_THROUGH_EXIT: &str =
    "traits/runtime_local_named_dyn_mutable_pass_through_exit";
pub(crate) const RUNTIME_LOCAL_NAMED_DYN_BOOLEAN_PASS_THROUGH_EXIT: &str =
    "traits/runtime_local_named_dyn_boolean_pass_through_exit";
pub(crate) const RUNTIME_LOCAL_NAMED_DYN_MUTABLE_BOOLEAN_PASS_THROUGH_EXIT: &str =
    "traits/runtime_local_named_dyn_mutable_boolean_pass_through_exit";
pub(crate) const RUNTIME_LOCAL_NAMED_DYN_MUTABLE_PROJECTED_BOOLEAN_PASS_THROUGH_EXIT: &str =
    "traits/runtime_local_named_dyn_mutable_projected_boolean_pass_through_exit";
pub(crate) const RUNTIME_LOCAL_NAMED_DYN_MULTI_HOP_PASS_THROUGH_EXIT: &str =
    "traits/runtime_local_named_dyn_multi_hop_pass_through_exit";

pub(crate) const PASS_CANARIES: &[&str] = &[
    RUNTIME_INDEXED_COPY_AGGREGATE_HANDOFF_EXIT,
    RUNTIME_MUTABLE_CALL_BEFORE_TRANSITION_ARGS_EXIT,
    RUNTIME_REFERENCED_LOCAL_OUTLIVES_SIBLING_GUARD_CALL_EXIT,
    RUNTIME_VIEW_LINKED_INPUT_UNRELATED_REF_WRITE_EXIT,
    RUNTIME_VALUE_CALL_SINGLE_EXECUTION_EXIT,
    RUNTIME_EXPLICIT_DISCARD_EXECUTES_EXIT,
    RUNTIME_TRANSITION_SUBJECT_CALL_SINGLE_EVALUATION_EXIT,
    RUNTIME_NONPLACE_RECORD_PATTERN_SINGLE_EVALUATION_EXIT,
    RUNTIME_EFFECTFUL_SUBJECT_SINGLE_EVALUATION_EXIT,
    RUNTIME_STATEMENT_CALL_SINGLE_EXECUTION_EXIT,
    RUNTIME_ASSIGNMENT_CALL_POST_MUTATION_VALUE_EXIT,
    RUNTIME_VALUE_CALL_RETURN_TYPES_EXIT,
    RUNTIME_VALUE_CALL_STRUCT_RESULT_TO_TARGET_EXIT,
    RUNTIME_VALUE_CALL_SELF_FIELD_ENUM_MATCH_EXIT,
    RUNTIME_VALUE_CALL_STRUCT_LITERAL_ARMS_EXIT,
    RUNTIME_CONTAINED_MACHINE_EXIT,
    RUNTIME_CALL_RESULT_AFTER_SPLICE_MUTATION_EXIT,
    RUNTIME_CALLED_MACHINE_LOOP_SEARCH_EXIT,
    RUNTIME_TRAILING_LOCAL_RETURN_EXIT,
    RUNTIME_LOOPING_VALUE_RETURN_EXIT,
    RUNTIME_LOOPING_CAST_RETURN_EXIT,
    RUNTIME_VALUE_CALL_SLICE_LEN_GUARD_EXIT,
    RUNTIME_SLEEP_EXIT,
    RUNTIME_WRITE_NO_NEWLINE_EXIT,
    RUNTIME_EXIT_CODE_EXIT,
    BORROW_CARRYING_DATA_FIELD_EXIT,
    RUNTIME_U8_FIELD_ARITH_EXIT,
    RUNTIME_I8_SIGNED_ARITH_EXIT,
    RUNTIME_I16_SIGNED_ARITH_EXIT,
    RUNTIME_U16_FIELD_ARITH_EXIT,
    RUNTIME_ADDR_FIELD_EXIT,
    RUNTIME_I64_SIGNED_ARITH_EXIT,
    RUNTIME_ADDR_VALUE_FLOW_EXIT,
    RUNTIME_ADDR_ALGEBRA_EXIT,
    RUNTIME_REF_PARAM_METHOD_DISPATCH_EXIT,
    RUNTIME_TYPED_TWO_METHOD_RECEIVERS_EXIT,
    RUNTIME_DYN_SINGLE_IMPL_DISPATCH_EXIT,
    RUNTIME_LOCAL_NAMED_DYN_DEVIRTUALIZED_EXIT,
    RUNTIME_LOCAL_NAMED_DYN_PASS_THROUGH_EXIT,
    RUNTIME_LOCAL_NAMED_DYN_UNIT_MULTI_HOP_RETURN,
    RUNTIME_LOCAL_NAMED_DYN_REBOUND_DIRECT_EXIT,
    RUNTIME_LOCAL_NAMED_DYN_STORED_RETURN,
    RUNTIME_LOCAL_NAMED_DYN_STORED_EXIT,
    RUNTIME_DYN_TWO_IMPL_DISPATCH_EXIT,
    RUNTIME_DYN_TWO_IMPL_DISPATCH_SWAPPED_EXIT,
    RUNTIME_ALIAS_WRITE_THROUGH_GUARDED_TRANSITION_EXIT,
    RUNTIME_REFERENCE_PARAM_FORWARDED_THROUGH_LOOP_EXIT,
    RUNTIME_VALUE_CALL_THROUGH_ALIAS_IN_DISPATCH_EXIT,
    RUNTIME_NESTED_VALUE_CALL_IN_SUBSTATE_EXIT,
    RUNTIME_CALL_IN_INLINED_SUBSTATE_EXIT,
    RUNTIME_ALIAS_INDEXED_READ_THROUGH_TRANSITION_EXIT,
    RUNTIME_DISPATCH_BINARY_CALL_ARGUMENT_EXIT,
    RUNTIME_DISPATCH_RESULT_FIELD_BINDING_EXIT,
    RUNTIME_TRAILING_STATE_MUT_PARAM_PHASE_EXIT,
    RUNTIME_SAME_TYPE_SECOND_RECEIVER_MUTATION_EXIT,
    RUNTIME_DISPATCH_FLOAT_TERMINAL_EXIT,
    RUNTIME_VALUE_MACHINE_RECEIVER_FIELD_POSTENTRY_EXIT,
    RUNTIME_NESTED_RECEIVER_SAME_TYPE_EXIT,
    RUNTIME_DISPATCH_SECOND_RECEIVER_EXIT,
    RUNTIME_DISPATCH_SIBLING_VALUE_CALLS_EXIT,
    RUNTIME_INLINE_REPEATED_RECEIVER_VALUE_CALLS_EXIT,
    RUNTIME_NONENTRY_SECOND_RECEIVER_EXIT,
    RUNTIME_SELFCALL_CHAIN_SECOND_RECEIVER_EXIT,
    RUNTIME_NESTED_INLINE_CHAIN_RESULT_EXIT,
    RUNTIME_NONENTRY_INLINE_SECOND_RECEIVER_EXIT,
    RUNTIME_NESTED_LOCAL_TERMINAL_SECOND_INSTANCE_EXIT,
    RUNTIME_NESTED_FIELD_TERMINAL_SECOND_INSTANCE_EXIT,
    RUNTIME_MULTIARM_SAME_NAMED_LOCALS_EXIT,
    RUNTIME_MULTIARM_TEXTEQ_LOCAL_EXIT,
    RUNTIME_PRE_GUARD_TEXTEQ_LOCAL_GUARD_EXIT,
    RUNTIME_PRE_GUARD_TEXTEQ_LOCAL_ARG_FORWARD_EXIT,
    RUNTIME_PARAM_RECEIVER_SECOND_INSTANCE_EXIT,
    RUNTIME_PARAM_FORWARD_CHAIN_SECOND_RECEIVER_EXIT,
    RUNTIME_MAIN_SOURCE_BUILDER_IS_ORDINARY_EXIT,
    RUNTIME_SATURATING_TIME_ARITH_EXIT,
    RUNTIME_NATURAL_TERMINATION_EXIT,
    RUNTIME_DEEP_STATE_NAME_COLLISION_EXIT,
    RUNTIME_U64_LITERAL_LET_GUARD_EXIT,
    RUNTIME_PARAM_RECEIVER_SINGLE_INSTANCE_EXIT,
    RUNTIME_DISPATCH_RESULT_ALIAS_READ_EXIT,
    RUNTIME_DISPATCH_SLICE_ELEMENT_TERMINAL_EXIT,
    RUNTIME_DISPATCH_RESULT_BINARY_TERMINAL_EXIT,
    RUNTIME_DISPATCH_RESULT_MULTI_ARM_EXIT,
    RUNTIME_DISPATCH_RESULT_GUARD_SUBJECT_EXIT,
    RUNTIME_DISPATCH_RESULT_TRANSITION_ARG_EXIT,
    RUNTIME_DISPATCHED_EFFECTFUL_REENTRANT_EXIT,
    RUNTIME_DISPATCH_RESULT_ENUM_CASE_EXIT,
    RUNTIME_DISPATCH_MACHINE_ARRAY_SLICE_ARG_EXIT,
    RUNTIME_DISPATCH_RESULT_FIELD_TERMINAL_EXIT,
    RUNTIME_NESTED_CALLED_MACHINE_LOOP_EXIT,
    RUNTIME_STATE_LOOP_INDEXED_SEARCH_EXIT,
    RUNTIME_CALL_RESULT_THROUGH_REFERENCE_FIELD_EXIT,
    RUNTIME_STRING_CALL_RESULT_THROUGH_REFERENCE_FIELD_EXIT,
    RUNTIME_TWO_STRING_CALL_RESULTS_THROUGH_REFERENCE_FIELDS_EXIT,
    RUNTIME_OFFSET_STRING_CALL_RESULTS_THROUGH_REFERENCE_FIELDS_EXIT,
    RUNTIME_REFERENCE_RETURNED_SLICE_ELEMENT_WRITE_EXIT,
    RUNTIME_REFERENCE_RETURNED_SLICE_ELEMENT_THROUGH_PARAM_EXIT,
    RUNTIME_NESTED_GUARDED_REFERENCE_RETURNED_SLICE_ELEMENT_EXIT,
    RUNTIME_MUTABLE_LOCAL_INDEXED_PARAMETER_WRITE_EXIT,
    RUNTIME_MUTABLE_MACHINE_OWNED_LOCAL_INDEXED_PARAMETER_WRITE_EXIT,
    RUNTIME_MUTABLE_DYNAMIC_INDEXED_MACHINE_OWNED_PARAMETER_WRITE_EXIT,
    RUNTIME_DISPATCH_LOCAL_INDEX_BINARY_WRITE_EXIT,
    RUNTIME_DISPATCH_HELPER_LOCAL_ALIAS_ADD_EXIT,
    RUNTIME_SLICE_ALIAS_INDEXED_FIELD_WRITE_EXIT,
    RUNTIME_SLICE_INDEXED_BINARY_RMW_EXIT,
    RUNTIME_MUT_REF_FORWARD_EXIT,
    RUNTIME_LOCAL_SLICE_FORWARD_EXIT,
    F32_GUARD_CONST_ARITH_LANDED_EXIT,
    F32_ARG_CONST_ARITH_LANDED_EXIT,
    RUNTIME_LOCAL_NAMED_DYN_MUTABLE_PASS_THROUGH_EXIT,
    RUNTIME_LOCAL_NAMED_DYN_BOOLEAN_PASS_THROUGH_EXIT,
    RUNTIME_LOCAL_NAMED_DYN_MUTABLE_BOOLEAN_PASS_THROUGH_EXIT,
    RUNTIME_LOCAL_NAMED_DYN_MUTABLE_PROJECTED_BOOLEAN_PASS_THROUGH_EXIT,
    RUNTIME_LOCAL_NAMED_DYN_MULTI_HOP_PASS_THROUGH_EXIT,
];
