use crate::identity::NativeStringStorage;
use crate::plan::NativePlan;
use omega_typed_program::expression::Expression;
use omega_typed_program::statement::TransitionGuard;

pub(in crate::identity) fn count_expression_span_strings(
    span: omega_core::arena::HandleSpan<Expression>,
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    if let Some(expressions) = native_plan
        .runtime_branching_calls
        .target_arguments
        .span(span)
    {
        for expression in expressions {
            count_expression_strings(expression, storage);
        }
    }
}

pub(in crate::identity) fn count_guard_strings(
    guard: &TransitionGuard,
    storage: &mut NativeStringStorage,
) {
    if let TransitionGuard::When(expression) = guard {
        count_expression_strings(expression, storage);
    }
}

pub(in crate::identity) fn count_expression_strings(
    expression: &Expression,
    storage: &mut NativeStringStorage,
) {
    match expression {
        Expression::ArrayLiteral(values) => {
            for value in values {
                count_expression_strings(value, storage);
            }
        }
        Expression::Binary(binary) => {
            count_expression_strings(&binary.left, storage);
            count_expression_strings(&binary.right, storage);
        }
        Expression::Indexed(indexed) => {
            count_expression_strings(&indexed.collection, storage);
            count_expression_strings(&indexed.index, storage);
        }
        Expression::Mutable(expression) => count_expression_strings(expression, storage),
        Expression::StructLiteral(struct_literal) => {
            storage.count_program_name_identity(&struct_literal.type_name);
            for field in &struct_literal.fields {
                storage.count_program_name_identity(&field.name);
                count_expression_strings(&field.value, storage);
            }
        }
        Expression::Name(path) => {
            for name in path {
                storage.count_program_name_identity(name);
            }
        }
        Expression::String(value) => storage.count_payload(value),
        Expression::Boolean(_) | Expression::Float(_) | Expression::Integer(_) => {}
    }
}
