use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
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

/// A nested runtime-COLUMN indexed target -- `grid[anything][i]` where the outer/column index `i`
/// is a runtime value and the collection is itself an indexed place. The static-assignment and
/// constant-integer fast paths cannot lower this shape: they would silently NO-OP (the element
/// never updates). Refusing the fast-path classification lets it fall to a regular `Assignment`,
/// which is recorded as a runtime-storage write and reported cleanly by the storage blocker rather
/// than vanishing. A const column (`grid[i][0]`) resolves to a fixed offset and IS lowerable, and a
/// single index (`arr[i]`, `self.field[i]`) has a non-indexed collection, so neither is refused.
fn target_is_nested_runtime_indexed(table: &ExpressionTable, target: ExpressionHandle) -> bool {
    let ExpressionNode::Indexed(indexed) = table.expression(target) else {
        return false;
    };
    let mut index = indexed.index;
    while let ExpressionNode::Mutable(inner) = table.expression(index) {
        index = *inner;
    }
    if matches!(table.expression(index), ExpressionNode::Integer(_)) {
        return false;
    }
    let mut collection = indexed.collection;
    while let ExpressionNode::Mutable(inner) = table.expression(collection) {
        collection = *inner;
    }
    matches!(table.expression(collection), ExpressionNode::Indexed(_))
}

fn is_static_assignment(program: &CheckedTrees, assignment: TableAssignment) -> bool {
    if target_is_nested_runtime_indexed(&program.expression_table, assignment.target) {
        return false;
    }
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
