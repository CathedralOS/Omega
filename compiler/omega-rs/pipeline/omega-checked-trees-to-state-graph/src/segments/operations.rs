use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{ExpressionNode, ExpressionTable};
use omega_checked_trees::name::Identifier;
use omega_checked_trees::statement::{StatementNode, TableAssignment, TableCall};
use omega_state_graph::{OperationExpressionRefs, OperationKind, StateGraph};

use super::copy_statement_expression_span;

pub(super) fn operation_kind(
    program: &CheckedTrees,
    table_statement: &StatementNode,
) -> OperationKind {
    match table_statement {
        StatementNode::Assignment(assignment) if is_static_assignment(program, *assignment) => {
            OperationKind::StaticAssignment
        }
        StatementNode::Assignment(assignment)
            if is_constant_integer_assignment(program, *assignment) =>
        {
            OperationKind::ConstantIntegerAssignment
        }
        StatementNode::Assignment(_) => OperationKind::Assignment,
        StatementNode::Call(call) => OperationKind::Call {
            receiver_symbol: call.receiver_symbol,
            target_symbol: call.target_symbol,
            has_receiver: !program
                .statement_table
                .name_path_members(call.receiver)
                .is_empty(),
            receiver: statement_call_receiver_name(program, call),
            target: call.target.clone(),
        },
        StatementNode::Expression(_) => OperationKind::Expression,
        StatementNode::LocalData(_) => OperationKind::LocalData,
        StatementNode::Transition(_) => unreachable!("transitions are not operations"),
    }
}

pub(super) fn operation_expression_refs(
    statement: &StatementNode,
    source_expressions: &ExpressionTable,
    state_graph: &mut StateGraph,
    statement_table: &omega_checked_trees::statement::StatementTable,
) -> OperationExpressionRefs {
    match statement {
        StatementNode::Assignment(assignment) => OperationExpressionRefs::Assignment {
            target: state_graph
                .expressions
                .copy_from(source_expressions, assignment.target),
            value: state_graph
                .expressions
                .copy_from(source_expressions, assignment.value),
        },
        StatementNode::Call(call) => OperationExpressionRefs::Call {
            arguments: copy_statement_expression_span(
                state_graph,
                source_expressions,
                statement_table,
                call.arguments,
            ),
        },
        StatementNode::Expression(expression) => OperationExpressionRefs::Expression(
            state_graph
                .expressions
                .copy_from(source_expressions, *expression),
        ),
        StatementNode::LocalData(local_data) if local_data.initial_value.is_valid() => {
            OperationExpressionRefs::Expression(
                state_graph
                    .expressions
                    .copy_from(source_expressions, local_data.initial_value),
            )
        }
        StatementNode::LocalData(_) | StatementNode::Transition(_) => OperationExpressionRefs::None,
    }
}

fn statement_call_receiver_name(program: &CheckedTrees, call: &TableCall) -> Identifier {
    let receiver = program.statement_table.name_path_members(call.receiver);
    receiver
        .last()
        .cloned()
        .unwrap_or_else(|| Identifier::generated_static("self"))
}

fn is_static_assignment(program: &CheckedTrees, assignment: TableAssignment) -> bool {
    let target_is_place = matches!(
        program.expression_table.expression(assignment.target),
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_)
    );
    let value_is_static = match program.expression_table.expression(assignment.value) {
        ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => true,
        ExpressionNode::Indexed(_) => true,
        ExpressionNode::Name(path) => {
            program
                .expression_table
                .name_path_members(path.members)
                .len()
                > 1
        }
        _ => false,
    };

    target_is_place && value_is_static
}

fn is_constant_integer_assignment(program: &CheckedTrees, assignment: TableAssignment) -> bool {
    matches!(
        program.expression_table.expression(assignment.target),
        ExpressionNode::Name(path) if program.expression_table.name_path_members(path.members).len() == 1
    ) && matches!(
        program.expression_table.expression(assignment.value),
        ExpressionNode::Integer(_)
    )
}
