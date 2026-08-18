mod aarch64;
mod shared;
mod x86_64;

#[cfg(test)]
pub(super) use aarch64::aarch64_csel;
pub(super) use aarch64::{
    emit_aarch64_adjust_sp, emit_aarch64_boolean_condition_value, emit_aarch64_boolean_control,
    emit_aarch64_boolean_expression, emit_aarch64_boolean_not_parameter_return,
    emit_aarch64_boolean_return, emit_aarch64_condition_load,
    emit_aarch64_conditional_boolean_control, emit_aarch64_conditional_boolean_expression_control,
    emit_aarch64_conditional_integer_control, emit_aarch64_conditional_integer_expression_control,
    emit_aarch64_integer_expression, emit_aarch64_parameter_return, emit_aarch64_return,
};
pub(super) use shared::{
    accountable_conditional_boolean_expression, accountable_direct_integer_expression,
    collect_scalar_stack_evidence, collect_x86_division_branch_evidence,
    conditional_with_terminal_shape, direct_conditional_boolean_shape,
    direct_conditional_integer_shape, emit_boolean_shared_convergence, emit_native_crash,
    integer_bits, linear_boolean_expression, require_native_integer_width,
};
pub(super) use x86_64::{
    emit_x86_64_adjust_sp, emit_x86_64_boolean_condition_value, emit_x86_64_boolean_control,
    emit_x86_64_boolean_expression, emit_x86_64_boolean_not_parameter_return,
    emit_x86_64_boolean_return, emit_x86_64_conditional_boolean_control,
    emit_x86_64_conditional_boolean_expression_control, emit_x86_64_conditional_integer_control,
    emit_x86_64_conditional_integer_expression_control, emit_x86_64_integer_expression,
    emit_x86_64_memory_load_width, emit_x86_64_parameter_return, emit_x86_64_return,
    emit_x86_64_stack_load, emit_x86_64_stack_load_width, emit_x86_64_stack_store,
    emit_x86_64_stack_store_width,
};
