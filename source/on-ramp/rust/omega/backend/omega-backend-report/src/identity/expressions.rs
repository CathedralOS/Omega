use crate::BackendReportInput;
use crate::identity::BackendStringStorage;
use psi_arena::HandleSpan;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

pub(in crate::identity) fn count_expression_span_strings(
    span: psi_arena::HandleSpan<ExpressionHandle>,
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
        ExpressionNode::Atomic(atomic) => {
            count_control_flow_expression_strings(table, atomic.value, storage)
        }
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
            for name in table.name_path_members(cast.target_label) {
                storage.count_program_name_identity(name);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            count_control_flow_expression_strings(table, indexed.collection, storage);
            count_control_flow_expression_strings(table, indexed.index, storage);
        }
        ExpressionNode::Range(range) => {
            count_control_flow_expression_strings(table, range.start, storage);
            count_control_flow_expression_strings(table, range.end, storage);
        }
        ExpressionNode::Member(member) => {
            count_control_flow_expression_strings(table, member.receiver, storage);
            storage.count_program_name_identity(&member.member);
        }
        ExpressionNode::Borrow(expression) => {
            count_control_flow_expression_strings(table, expression.target, storage);
        }
        ExpressionNode::Unary(unary) => {
            count_control_flow_expression_strings(table, unary.operand, storage);
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
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
