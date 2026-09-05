//! Exact corpus inputs used by the dedicated value and type checks.
//! Checked, native, cross-target, and inline diagnostic assertions stay in the tests.

pub(crate) const WRITE_ONLY_WHOLE_SCALAR_REPLACE: &str = "borrow/write_only_whole_scalar_replace";
pub(crate) const WRITE_ONLY_FIXED_BYTE_ELEMENT: &str = "borrow/write_only_fixed_byte_element";
pub(crate) const WRITE_ONLY_RECORD_FIELD_REPLACE: &str = "borrow/write_only_record_field_replace";
pub(crate) const WRITE_ONLY_NESTED_RECORD_FIELD_REPLACE: &str =
    "borrow/write_only_nested_record_field_replace";
pub(crate) const WRITE_ONLY_RECORD_WHOLE_ROOT_REPLACEMENT: &str =
    "borrow/write_only_record_whole_root_replacement";
pub(crate) const WRITE_ONLY_RECORD_FIELD_OBSERVATION: &str =
    "borrow/write_only_record_field_observation";
pub(crate) const WRITE_ONLY_CONSTRAINED_RECORD_FIELD: &str =
    "borrow/write_only_constrained_record_field";
pub(crate) const WRITE_ONLY_DYNAMIC_BYTE_INDEX: &str = "borrow/write_only_dynamic_byte_index";
pub(crate) const WRITE_ONLY_UNBOUNDED_DYNAMIC_BYTE_INDEX: &str =
    "borrow/write_only_unbounded_dynamic_byte_index";
pub(crate) const WRITE_ONLY_BYTE_INDEX_OUT_OF_BOUNDS: &str =
    "borrow/write_only_byte_index_out_of_bounds";
pub(crate) const WRITE_ONLY_BYTE_RANGE: &str = "borrow/write_only_byte_range";
pub(crate) const WRITE_ONLY_BYTE_RANGE_WIDTH: &str = "borrow/write_only_byte_range_width";
pub(crate) const WRITE_ONLY_BYTE_OBSERVATION: &str = "borrow/write_only_byte_observation";
pub(crate) const WRITE_ONLY_OBSERVATION: &str = "borrow/write_only_observation";
pub(crate) const WRITE_ONLY_MUTABLE_WIDENING: &str = "borrow/write_only_mutable_widening";
pub(crate) const WRITE_ONLY_IMPLICIT_ATTENUATION: &str = "borrow/write_only_implicit_attenuation";
pub(crate) const WRITE_ONLY_PROVIDER_GATE: &str = "borrow/write_only_provider_gate";
pub(crate) const VALUE_CALL_AS_HOST_ARG_EXIT: &str = "calls/value_call_as_host_arg_exit";
pub(crate) const COMPUTED_HOST_ARG_EXIT: &str = "calls/computed_host_arg_exit";
pub(crate) const COMPUTED_HOST_CAST_ARG_EXIT: &str = "calls/computed_host_cast_arg_exit";
pub(crate) const COMPUTED_HOST_BUILTIN_ARG_EXIT: &str = "calls/computed_host_builtin_arg_exit";
pub(crate) const COMPUTED_HOST_INDEXED_ARG_EXIT: &str = "calls/computed_host_indexed_arg_exit";
pub(crate) const EXACT_OVERFLOW_VALUE_CALL_HINT: &str = "arithmetic/exact_overflow_value_call_hint";
pub(crate) const UNKNOWN_FIELD_WRITE_REJECTED: &str = "arithmetic/unknown_field_write_rejected";
pub(crate) const LITERAL_CLASS_MISMATCH_REJECTED: &str =
    "arithmetic/literal_class_mismatch_rejected";
pub(crate) const MEMBER_CLASS_MISMATCH_REJECTED: &str = "arithmetic/member_class_mismatch_rejected";
pub(crate) const ARG_CLASS_MISMATCH_REJECTED: &str = "arithmetic/arg_class_mismatch_rejected";
pub(crate) const VALUE_CALL_ARG_CLASS_MISMATCH_REJECTED: &str =
    "arithmetic/value_call_arg_class_mismatch_rejected";
pub(crate) const NARROWING_CALL_ARG_REJECTED: &str = "arithmetic/narrowing_call_arg_rejected";
pub(crate) const NARROWING_VALUE_CALL_ARG_REJECTED: &str =
    "arithmetic/narrowing_value_call_arg_rejected";
pub(crate) const TRANSITION_VALUE_OVERFLOW_REJECTED: &str =
    "arithmetic/transition_value_overflow_rejected";
pub(crate) const JOINT_ADD_GUARD_WRONG_OPERAND: &str = "arithmetic/joint_add_guard_wrong_operand";
pub(crate) const STRUCT_LITERAL_CLASS_MISMATCH_REJECTED: &str =
    "arithmetic/struct_literal_class_mismatch_rejected";
pub(crate) const STRUCT_LITERAL_NARROWING_REJECTED: &str =
    "arithmetic/struct_literal_narrowing_rejected";
pub(crate) const ARRAY_LITERAL_ELEMENT_NARROWING_REJECTED: &str =
    "arithmetic/array_literal_element_narrowing_rejected";
pub(crate) const ARRAY_LITERAL_LET_INIT_NARROWING_REJECTED: &str =
    "arithmetic/array_literal_let_init_narrowing_rejected";
pub(crate) const LET_INIT_CLASS_MISMATCH_REJECTED: &str =
    "arithmetic/let_init_class_mismatch_rejected";
pub(crate) const RETURN_VALUE_CLASS_MISMATCH_REJECTED: &str =
    "arithmetic/return_value_class_mismatch_rejected";
pub(crate) const TERMINAL_RETURN_CLASS_MISMATCH_REJECTED: &str =
    "arithmetic/terminal_return_class_mismatch_rejected";
pub(crate) const WRONG_STRUCT_TYPE_ARGUMENT_REJECTED: &str =
    "arithmetic/wrong_struct_type_argument_rejected";
pub(crate) const WRONG_STRUCT_TYPE_ASSIGNMENT_REJECTED: &str =
    "arithmetic/wrong_struct_type_assignment_rejected";
pub(crate) const WRONG_STRUCT_TYPE_ARRAY_ELEMENT_REJECTED: &str =
    "arithmetic/wrong_struct_type_array_element_rejected";
pub(crate) const UNKNOWN_FIELD_READ_REJECTED: &str = "arithmetic/unknown_field_read_rejected";

// The executing tests use named cases; corpus inventory consumes this slice.
pub(crate) const PASS_CANARIES: &[&str] = &[
    WRITE_ONLY_WHOLE_SCALAR_REPLACE,
    WRITE_ONLY_FIXED_BYTE_ELEMENT,
    WRITE_ONLY_RECORD_FIELD_REPLACE,
    WRITE_ONLY_NESTED_RECORD_FIELD_REPLACE,
    WRITE_ONLY_DYNAMIC_BYTE_INDEX,
    WRITE_ONLY_BYTE_RANGE,
    VALUE_CALL_AS_HOST_ARG_EXIT,
    COMPUTED_HOST_ARG_EXIT,
    COMPUTED_HOST_CAST_ARG_EXIT,
    COMPUTED_HOST_BUILTIN_ARG_EXIT,
    COMPUTED_HOST_INDEXED_ARG_EXIT,
];

// The executing tests use named cases; corpus inventory consumes this slice.
pub(crate) const FAIL_CANARIES: &[&str] = &[
    WRITE_ONLY_RECORD_WHOLE_ROOT_REPLACEMENT,
    WRITE_ONLY_RECORD_FIELD_OBSERVATION,
    WRITE_ONLY_CONSTRAINED_RECORD_FIELD,
    WRITE_ONLY_UNBOUNDED_DYNAMIC_BYTE_INDEX,
    WRITE_ONLY_BYTE_INDEX_OUT_OF_BOUNDS,
    WRITE_ONLY_BYTE_RANGE_WIDTH,
    WRITE_ONLY_BYTE_OBSERVATION,
    WRITE_ONLY_OBSERVATION,
    WRITE_ONLY_MUTABLE_WIDENING,
    WRITE_ONLY_IMPLICIT_ATTENUATION,
    WRITE_ONLY_PROVIDER_GATE,
    EXACT_OVERFLOW_VALUE_CALL_HINT,
    UNKNOWN_FIELD_WRITE_REJECTED,
    LITERAL_CLASS_MISMATCH_REJECTED,
    MEMBER_CLASS_MISMATCH_REJECTED,
    ARG_CLASS_MISMATCH_REJECTED,
    VALUE_CALL_ARG_CLASS_MISMATCH_REJECTED,
    NARROWING_CALL_ARG_REJECTED,
    NARROWING_VALUE_CALL_ARG_REJECTED,
    TRANSITION_VALUE_OVERFLOW_REJECTED,
    JOINT_ADD_GUARD_WRONG_OPERAND,
    STRUCT_LITERAL_CLASS_MISMATCH_REJECTED,
    STRUCT_LITERAL_NARROWING_REJECTED,
    ARRAY_LITERAL_ELEMENT_NARROWING_REJECTED,
    ARRAY_LITERAL_LET_INIT_NARROWING_REJECTED,
    LET_INIT_CLASS_MISMATCH_REJECTED,
    RETURN_VALUE_CLASS_MISMATCH_REJECTED,
    TERMINAL_RETURN_CLASS_MISMATCH_REJECTED,
    WRONG_STRUCT_TYPE_ARGUMENT_REJECTED,
    WRONG_STRUCT_TYPE_ASSIGNMENT_REJECTED,
    WRONG_STRUCT_TYPE_ARRAY_ELEMENT_REJECTED,
    UNKNOWN_FIELD_READ_REJECTED,
];
