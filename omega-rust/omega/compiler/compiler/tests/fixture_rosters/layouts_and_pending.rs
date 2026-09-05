//! Fixture identities shared with the executing owner and corpus inventory.

pub const RUNTIME_PLAN_LAID_VALUE_FIELD_EXIT: &str = "layouts/runtime_plan_laid_value_field_exit";
pub const RUNTIME_PLAN_LAID_ERASED_FIELD_EXIT: &str = "layouts/runtime_plan_laid_erased_field_exit";
pub const RUNTIME_DISTINCT_CLOSED_ERASED_SUMS_EXIT: &str =
    "generics/runtime_distinct_closed_erased_sums_exit";
pub const RUNTIME_MIXED_GENERIC_ERASED_SUM_EXIT: &str =
    "generics/runtime_mixed_generic_erased_sum_exit";
pub const RUNTIME_GENERIC_EXACT_CALL_RETURN_EXIT: &str =
    "generics/runtime_generic_exact_call_return_exit";
pub const RUNTIME_WIRE_ERASED_FIELD_ROUNDTRIP_EXIT: &str =
    "wire/runtime_wire_erased_field_roundtrip_exit";
pub const RUNTIME_WIRE_NESTED_ERASED_FIELD_ROUNDTRIP_EXIT: &str =
    "wire/runtime_wire_nested_erased_field_roundtrip_exit";
pub const RUNTIME_PLAN_LAID_COMPACT_BITS_EXIT: &str = "layouts/runtime_plan_laid_compact_bits_exit";
pub const RUNTIME_PLAN_LAID_INTEGER_AT_PROJECTION_EXIT: &str =
    "layouts/runtime_plan_laid_integer_at_projection_exit";
pub const RUNTIME_PLAN_LAID_INTEGER_AT_TOTAL_WRITE_EXIT: &str =
    "layouts/runtime_plan_laid_integer_at_total_write_exit";
pub const RUNTIME_PLAN_LAID_INTEGER_AT_PROVED_WRITE_EXIT: &str =
    "layouts/runtime_plan_laid_integer_at_proved_write_exit";
pub const RUNTIME_PLAN_LAID_VALUE_BY_VALUE_PARAM_EXIT: &str =
    "layouts/runtime_plan_laid_value_by_value_param_exit";
pub const RUNTIME_PLAN_LAID_RECORD_VIEW_EXIT: &str = "layouts/runtime_plan_laid_record_view_exit";
pub const RUNTIME_PLAN_LAID_FIXED_ARRAY_VIEW_EXIT: &str =
    "layouts/runtime_plan_laid_fixed_array_view_exit";
pub const RUNTIME_PLAN_LAID_FIXED_ARRAY_MUTABLE_WRITE_EXIT: &str =
    "layouts/runtime_plan_laid_fixed_array_mutable_write_exit";
pub const RUNTIME_PLAN_LAID_NESTED_FIXED_ARRAY_MUTABLE_WRITE_EXIT: &str =
    "layouts/runtime_plan_laid_nested_fixed_array_mutable_write_exit";
pub const RUNTIME_PLAN_LAID_NESTED_RECORD_MUTABLE_WRITE_EXIT: &str =
    "layouts/runtime_plan_laid_nested_record_mutable_write_exit";
pub const RUNTIME_PLAN_LAID_RECORD_ARRAY_MUTABLE_WRITE_EXIT: &str =
    "layouts/runtime_plan_laid_record_array_mutable_write_exit";
pub const RUNTIME_PLAN_LAID_RECORD_MUTABLE_WRITE_EXIT: &str =
    "layouts/runtime_plan_laid_record_mutable_write_exit";
pub const VALUE_CALL_SEQUENTIAL_RESULT_SLOTS_EXIT: &str =
    "calls/value_call_sequential_result_slots_exit";
pub const ARITHMETIC_DOMAIN_SATURATING_CONST_FOLD_EXIT: &str =
    "expressions/arithmetic_domain_saturating_const_fold_exit";
pub const VALUE_CALL_SEQUENTIAL_SELF_CAPTURE_EXIT: &str =
    "calls/value_call_sequential_self_capture_exit";
pub const RUNTIME_F64_STATE_ARG_EXIT: &str = "expressions/runtime_f64_state_arg_exit";
pub const RUNTIME_LET_LOCAL_NESTED_STATE_ARG_EXIT: &str =
    "calls/runtime_let_local_nested_state_arg_exit";

pub const PASS_CANARIES: &[&str] = &[
    RUNTIME_PLAN_LAID_VALUE_FIELD_EXIT,
    RUNTIME_PLAN_LAID_ERASED_FIELD_EXIT,
    RUNTIME_DISTINCT_CLOSED_ERASED_SUMS_EXIT,
    RUNTIME_MIXED_GENERIC_ERASED_SUM_EXIT,
    RUNTIME_GENERIC_EXACT_CALL_RETURN_EXIT,
    RUNTIME_WIRE_ERASED_FIELD_ROUNDTRIP_EXIT,
    RUNTIME_WIRE_NESTED_ERASED_FIELD_ROUNDTRIP_EXIT,
    RUNTIME_PLAN_LAID_COMPACT_BITS_EXIT,
    RUNTIME_PLAN_LAID_INTEGER_AT_PROJECTION_EXIT,
    RUNTIME_PLAN_LAID_INTEGER_AT_TOTAL_WRITE_EXIT,
    RUNTIME_PLAN_LAID_INTEGER_AT_PROVED_WRITE_EXIT,
    RUNTIME_PLAN_LAID_VALUE_BY_VALUE_PARAM_EXIT,
    RUNTIME_PLAN_LAID_RECORD_VIEW_EXIT,
    RUNTIME_PLAN_LAID_FIXED_ARRAY_VIEW_EXIT,
    RUNTIME_PLAN_LAID_FIXED_ARRAY_MUTABLE_WRITE_EXIT,
    RUNTIME_PLAN_LAID_NESTED_FIXED_ARRAY_MUTABLE_WRITE_EXIT,
    RUNTIME_PLAN_LAID_NESTED_RECORD_MUTABLE_WRITE_EXIT,
    RUNTIME_PLAN_LAID_RECORD_ARRAY_MUTABLE_WRITE_EXIT,
    RUNTIME_PLAN_LAID_RECORD_MUTABLE_WRITE_EXIT,
    VALUE_CALL_SEQUENTIAL_RESULT_SLOTS_EXIT,
    ARITHMETIC_DOMAIN_SATURATING_CONST_FOLD_EXIT,
    VALUE_CALL_SEQUENTIAL_SELF_CAPTURE_EXIT,
    RUNTIME_F64_STATE_ARG_EXIT,
    RUNTIME_LET_LOCAL_NESTED_STATE_ARG_EXIT,
];
