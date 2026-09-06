//! Corpus inputs used by the provider, float, and console tests.
//! Native, checked, cross-target, and mutation assertions remain with their owners.

pub(crate) const RUNTIME_ADAPTER_DISPATCH_EXIT: &str = "providers/runtime_adapter_dispatch_exit";
pub(crate) const CHECKED_BOUNDARY_OPERATOR_DISPATCH_EXIT: &str =
    "providers/checked_boundary_operator_dispatch_exit";
pub(crate) const CHECKED_FIXED_OPERATOR_DISPATCH_EXIT: &str =
    "providers/checked_fixed_operator_dispatch_exit";
pub(crate) const CHECKED_BOUNDARY_OPERATOR_PHYSICAL_CUSTODY: &str =
    "providers/checked_boundary_operator_physical_custody";
pub(crate) const CHECKED_FIXED_OPERATOR_PHYSICAL_CUSTODY: &str =
    "providers/checked_fixed_operator_physical_custody";
pub(crate) const CHECKED_OPERATOR_FRAGMENT_PUBLICATION: &str =
    "providers/checked_operator_fragment_publication";
pub(crate) const SPECIALIZED_BOUNDARY_OPERATOR_PHYSICAL_CUSTODY: &str =
    "providers/specialized_boundary_operator_physical_custody";
pub(crate) const SPECIALIZED_FIXED_OPERATOR_PHYSICAL_CUSTODY: &str =
    "providers/specialized_fixed_operator_physical_custody";
pub(crate) const SPECIALIZED_STRUCTURAL_FIXED_OPERATOR_TERMINAL_CUSTODY: &str =
    "providers/specialized_structural_fixed_operator_terminal_custody";
pub(crate) const NESTED_CHECKED_BOUNDARY_OPERATOR_PHYSICAL_CUSTODY: &str =
    "providers/nested_checked_boundary_operator_physical_custody";
pub(crate) const RUNTIME_RESULT_DOMAIN_REQUIREMENT_OVERLOAD_EXIT: &str =
    "providers/runtime_result_domain_requirement_overload_exit";
pub(crate) const PROVIDER_TYPE_SLOT_SELECTED: &str = "providers/provider_type_slot_selected";
pub(crate) const SERVICE_FUSED_ERASURE_COMPILE: &str = "providers/service_fused_erasure_compile";
pub(crate) const SERVICE_FUSED_ROOT_ESTABLISHMENT: &str =
    "providers/service_fused_root_establishment";
pub(crate) const PROVIDER_TYPE_TARGET_DEFAULT: &str = "providers/provider_type_target_default";
pub(crate) const COMPONENT_OWNER_PROVIDER_OVERRIDE_COMPILE: &str =
    "providers/component_owner_provider_override_compile";
pub(crate) const TEST_OWNER_PROVIDER_OVERRIDE_COMPILE: &str =
    "providers/test_owner_provider_override_compile";
pub(crate) const PROVIDER_TYPE_TARGET_DEFAULT_OVERRIDE: &str =
    "providers/provider_type_target_default_override";
pub(crate) const ADAPTER_SATISFIES_COMPILE: &str = "providers/adapter_satisfies_compile";
pub(crate) const EXTERNAL_LEAF_VIA_COMPILE: &str = "providers/external_leaf_via_compile";
pub(crate) const EXTERNAL_LEAF_DLLIMPORT_COMPILE: &str =
    "providers/external_leaf_dllimport_compile";
pub(crate) const RUNTIME_ADAPTER_FORWARDING_EXIT: &str =
    "providers/runtime_adapter_forwarding_exit";
pub(crate) const ASM_PORT_OUT_FINAL_VALIDATION: &str = "inline_asm/asm_port_out_final_validation";
pub(crate) const RUNTIME_BOUNDARY_CAPABILITY_STATE_FORWARDING_EXIT: &str =
    "providers/runtime_boundary_capability_state_forwarding_exit";
pub(crate) const RUNTIME_CONSOLE_BYTE_LITERAL_EXIT: &str = "host/runtime_console_byte_literal_exit";
pub(crate) const RUNTIME_CONSOLE_BYTE_READ_RETURN: &str = "host/runtime_console_byte_read_return";
pub(crate) const RUNTIME_CONSOLE_BYTE_INSPECTION_EXIT: &str =
    "host/runtime_console_byte_inspection_exit";
pub(crate) const RUNTIME_CONSOLE_BYTE_ECHO_EXIT: &str = "host/runtime_console_byte_echo_exit";
pub(crate) const RUNTIME_IMPORT_CALL_ARGUMENT_EXIT: &str =
    "providers/runtime_import_call_argument_exit";
pub(crate) const MUTUAL_CYCLE_TAIL_ADMITTED_EXIT: &str = "calls/mutual_cycle_tail_admitted_exit";
pub(crate) const CONST_FOLD_UNSIGNED_LANDED_OPS_EXIT: &str =
    "arithmetic/const_fold_unsigned_landed_ops_exit";
pub(crate) const CONST_FOLD_UNSIGNED_SHIFT_RIGHT_ARG_EXIT: &str =
    "arithmetic/const_fold_unsigned_shift_right_arg_exit";
pub(crate) const CONST_FOLD_UNSIGNED_DIVIDE_ARG_EXIT: &str =
    "arithmetic/const_fold_unsigned_divide_arg_exit";
pub(crate) const UNSIGNED_MIN_MAX_WRAPPING_LOCAL_EXIT: &str =
    "arithmetic/unsigned_min_max_wrapping_local_exit";
pub(crate) const UNSIGNED_MIN_MAX_OPERAND_POSITION_EXIT: &str =
    "arithmetic/unsigned_min_max_operand_position_exit";
pub(crate) const SUFFIX_BOUNDARY_MAGNITUDES_EXIT: &str =
    "arithmetic/suffix_boundary_magnitudes_exit";
pub(crate) const FLOAT_VALUE_CALL_RETURN_EXIT: &str = "calls/float_value_call_return_exit";
pub(crate) const EXPANSION_FLOAT_LOCAL_GUARD_EXIT: &str = "float/expansion_float_local_guard_exit";
pub(crate) const FLOAT_VALUE_CALL_RUNTIME_ARG_EXIT: &str =
    "calls/float_value_call_runtime_arg_exit";
pub(crate) const F32_CHAIN_PER_OP_ROUNDING_EXIT: &str = "float/f32_chain_per_op_rounding_exit";
pub(crate) const RUNTIME_STD_IS_FINITE_EXIT: &str = "float/runtime_std_is_finite_exit";
pub(crate) const BOOL_VALUE_CALL_RETURN_EXIT: &str = "calls/bool_value_call_return_exit";
pub(crate) const STRUCT_LITERAL_TRANSITION_ARG_EXIT: &str =
    "calls/struct_literal_transition_arg_exit";
pub(crate) const RUNTIME_INDEXED_ELEMENT_COPY_WRITE_EXIT: &str =
    "slices/runtime_indexed_element_copy_write_exit";
pub(crate) const SUFFIX_LANDED_OPERAND_POSITION_EXIT: &str =
    "arithmetic/suffix_landed_operand_position_exit";
pub(crate) const SUFFIX_F32_SINGLE_ROUNDING_EXIT: &str = "float/suffix_f32_single_rounding_exit";
pub(crate) const UNSUFFIXED_F32_DESTINATION_SINGLE_ROUNDING_EXIT: &str =
    "float/unsuffixed_f32_destination_single_rounding_exit";
pub(crate) const UNSUFFIXED_F32_ARGUMENT_SINGLE_ROUNDING_EXIT: &str =
    "float/unsuffixed_f32_argument_single_rounding_exit";
pub(crate) const F32_PER_OPERATION_ROUNDING_EXIT: &str = "float/f32_per_operation_rounding_exit";
pub(crate) const ANONYMOUS_EXACT_RAT_CONST_EXIT: &str = "float/anonymous_exact_rat_const_exit";
pub(crate) const FINITE_CORE_DOMAIN_RANGE_DISCHARGE: &str =
    "float/finite_core_domain_range_discharge";
pub(crate) const STRUCT_LITERAL_FIELD_COERCION: &str = "arithmetic/struct_literal_field_coercion";
pub(crate) const ARRAY_ELEMENT_WRITE_WIDTH_DOMAIN: &str =
    "arithmetic/array_element_write_width_domain";
pub(crate) const INT_TRANSITION_ARG_WIDTH_WRAP: &str = "arithmetic/int_transition_arg_width_wrap";
pub(crate) const F32_TRANSITION_ARG_ROUNDING: &str = "arithmetic/f32_transition_arg_rounding";
pub(crate) const F32_FIELD_STORE_ROUNDING: &str = "arithmetic/f32_field_store_rounding";
pub(crate) const CONST_FOLD_CAST_SIGNEDNESS: &str = "arithmetic/const_fold_cast_signedness";
pub(crate) const RUNTIME_STDIN_LINE_BUFFERING_EXIT: &str = "text/runtime_stdin_line_buffering_exit";
pub(crate) const RUNTIME_CONSOLE_LINE_FIXED_ARRAY_EXIT: &str =
    "host/runtime_console_line_fixed_array_exit";
pub(crate) const RUNTIME_CONSOLE_LINE_DESCRIPTOR_EXIT: &str =
    "host/runtime_console_line_descriptor_exit";

pub(crate) const CONSOLE_LINE_REPLAY_CANARIES: &[&str] = &[
    RUNTIME_STDIN_LINE_BUFFERING_EXIT,
    RUNTIME_CONSOLE_LINE_FIXED_ARRAY_EXIT,
    RUNTIME_CONSOLE_LINE_DESCRIPTOR_EXIT,
];

pub(crate) const PASS_CANARIES: &[&str] = &[
    RUNTIME_ADAPTER_DISPATCH_EXIT,
    CHECKED_BOUNDARY_OPERATOR_DISPATCH_EXIT,
    CHECKED_FIXED_OPERATOR_DISPATCH_EXIT,
    CHECKED_BOUNDARY_OPERATOR_PHYSICAL_CUSTODY,
    CHECKED_FIXED_OPERATOR_PHYSICAL_CUSTODY,
    CHECKED_OPERATOR_FRAGMENT_PUBLICATION,
    SPECIALIZED_BOUNDARY_OPERATOR_PHYSICAL_CUSTODY,
    SPECIALIZED_FIXED_OPERATOR_PHYSICAL_CUSTODY,
    SPECIALIZED_STRUCTURAL_FIXED_OPERATOR_TERMINAL_CUSTODY,
    NESTED_CHECKED_BOUNDARY_OPERATOR_PHYSICAL_CUSTODY,
    RUNTIME_RESULT_DOMAIN_REQUIREMENT_OVERLOAD_EXIT,
    PROVIDER_TYPE_SLOT_SELECTED,
    SERVICE_FUSED_ERASURE_COMPILE,
    SERVICE_FUSED_ROOT_ESTABLISHMENT,
    PROVIDER_TYPE_TARGET_DEFAULT,
    COMPONENT_OWNER_PROVIDER_OVERRIDE_COMPILE,
    TEST_OWNER_PROVIDER_OVERRIDE_COMPILE,
    PROVIDER_TYPE_TARGET_DEFAULT_OVERRIDE,
    ADAPTER_SATISFIES_COMPILE,
    EXTERNAL_LEAF_VIA_COMPILE,
    EXTERNAL_LEAF_DLLIMPORT_COMPILE,
    RUNTIME_ADAPTER_FORWARDING_EXIT,
    ASM_PORT_OUT_FINAL_VALIDATION,
    RUNTIME_BOUNDARY_CAPABILITY_STATE_FORWARDING_EXIT,
    RUNTIME_CONSOLE_BYTE_LITERAL_EXIT,
    RUNTIME_CONSOLE_BYTE_READ_RETURN,
    RUNTIME_CONSOLE_BYTE_INSPECTION_EXIT,
    RUNTIME_CONSOLE_BYTE_ECHO_EXIT,
    RUNTIME_IMPORT_CALL_ARGUMENT_EXIT,
    MUTUAL_CYCLE_TAIL_ADMITTED_EXIT,
    CONST_FOLD_UNSIGNED_LANDED_OPS_EXIT,
    CONST_FOLD_UNSIGNED_SHIFT_RIGHT_ARG_EXIT,
    CONST_FOLD_UNSIGNED_DIVIDE_ARG_EXIT,
    UNSIGNED_MIN_MAX_WRAPPING_LOCAL_EXIT,
    UNSIGNED_MIN_MAX_OPERAND_POSITION_EXIT,
    SUFFIX_BOUNDARY_MAGNITUDES_EXIT,
    FLOAT_VALUE_CALL_RETURN_EXIT,
    EXPANSION_FLOAT_LOCAL_GUARD_EXIT,
    FLOAT_VALUE_CALL_RUNTIME_ARG_EXIT,
    F32_CHAIN_PER_OP_ROUNDING_EXIT,
    RUNTIME_STD_IS_FINITE_EXIT,
    BOOL_VALUE_CALL_RETURN_EXIT,
    STRUCT_LITERAL_TRANSITION_ARG_EXIT,
    RUNTIME_INDEXED_ELEMENT_COPY_WRITE_EXIT,
    SUFFIX_LANDED_OPERAND_POSITION_EXIT,
    SUFFIX_F32_SINGLE_ROUNDING_EXIT,
    UNSUFFIXED_F32_DESTINATION_SINGLE_ROUNDING_EXIT,
    UNSUFFIXED_F32_ARGUMENT_SINGLE_ROUNDING_EXIT,
    F32_PER_OPERATION_ROUNDING_EXIT,
    ANONYMOUS_EXACT_RAT_CONST_EXIT,
    FINITE_CORE_DOMAIN_RANGE_DISCHARGE,
    STRUCT_LITERAL_FIELD_COERCION,
    ARRAY_ELEMENT_WRITE_WIDTH_DOMAIN,
    INT_TRANSITION_ARG_WIDTH_WRAP,
    F32_TRANSITION_ARG_ROUNDING,
    F32_FIELD_STORE_ROUNDING,
    CONST_FOLD_CAST_SIGNEDNESS,
];
