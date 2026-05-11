use crate::BackendReportInput;
use crate::identity::BackendStringStorage;
use omega_core::arena::HandleSpan;
use omega_typed_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_typed_trees::statement::TransitionGuard;

pub(in crate::identity) fn count_expression_span_strings(
    span: omega_core::arena::HandleSpan<ExpressionHandle>,
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    if let Some(expression_handles) = backend_plan
        .runtime_branching_calls
        .target_arguments
        .span(span)
    {
        for expression in expression_handles {
            count_control_flow_expression_strings(
                &backend_plan.runtime_branching_calls.expressions,
                *expression,
                storage,
            );
        }
    }
}

pub(in crate::identity) fn count_guard_strings(
    guard: &TransitionGuard,
    storage: &mut BackendStringStorage,
) {
    if let TransitionGuard::When(expression) = guard {
        count_expression_strings(expression, storage);
    }
}

pub(in crate::identity) fn count_control_flow_expression_span_strings(
    table: &ExpressionTable,
    span: HandleSpan<ExpressionHandle>,
    storage: &mut BackendStringStorage,
) {
    for expression in table.expression_handles(span) {
        count_control_flow_expression_strings(table, *expression, storage);
    }
}

pub(in crate::identity) fn count_control_flow_expression_strings(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    storage: &mut BackendStringStorage,
) {
    match table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            count_control_flow_expression_span_strings(table, *values, storage);
        }
        ExpressionNode::Binary(binary) => {
            count_control_flow_expression_strings(table, binary.left, storage);
            count_control_flow_expression_strings(table, binary.right, storage);
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                count_control_flow_expression_strings(table, call.receiver, storage);
            }
            for argument in table.expression_handles(call.arguments) {
                count_control_flow_expression_strings(table, *argument, storage);
            }
            storage.count_program_name_identity(&call.target);
        }
        ExpressionNode::Cast(cast) => {
            count_control_flow_expression_strings(table, cast.value, storage);
            for name in table.name_path_members(cast.target_type) {
                storage.count_program_name_identity(name);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            count_control_flow_expression_strings(table, indexed.collection, storage);
            count_control_flow_expression_strings(table, indexed.index, storage);
        }
        ExpressionNode::Member(member) => {
            count_control_flow_expression_strings(table, member.receiver, storage);
            storage.count_program_name_identity(&member.member);
        }
        ExpressionNode::Mutable(expression) => {
            count_control_flow_expression_strings(table, *expression, storage);
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            storage.count_program_name_identity(&struct_literal.type_name);
            for field in table.struct_fields(struct_literal.fields) {
                storage.count_program_name_identity(&field.name);
                count_control_flow_expression_strings(table, field.value, storage);
            }
        }
        ExpressionNode::Name(path) => {
            for name in table.name_path_members(path.members) {
                storage.count_program_name_identity(name);
            }
        }
        ExpressionNode::String(value) => storage.count_payload(value),
        ExpressionNode::Boolean(_) | ExpressionNode::Float(_) | ExpressionNode::Integer(_) => {}
    }
}

pub(in crate::identity) fn count_expression_strings(
    expression: &Expression,
    storage: &mut BackendStringStorage,
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
        Expression::Call(call) => {
            if let Some(receiver) = &call.receiver {
                count_expression_strings(receiver, storage);
            }
            for argument in &call.arguments {
                count_expression_strings(argument, storage);
            }
            storage.count_program_name_identity(&call.target);
        }
        Expression::Cast(cast) => {
            count_expression_strings(&cast.value, storage);
            for name in &cast.target_type {
                storage.count_program_name_identity(name);
            }
        }
        Expression::Indexed(indexed) => {
            count_expression_strings(&indexed.collection, storage);
            count_expression_strings(&indexed.index, storage);
        }
        Expression::Member(member) => {
            count_expression_strings(&member.receiver, storage);
            storage.count_program_name_identity(&member.member);
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
