use omega_state_graph::{OperationExpressionRefs, OperationKind, StateGraph};
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_checked_trees::name::Identifier;
use psi_checked_trees::statement::{StatementNode, TableAssignment, TableCall};

use super::copy_statement_expression_span;
use crate::runtime_expressions::copy_runtime_expression;

pub(super) fn operation_kind(
    program: &CheckedTrees,
    table_statement: &StatementNode,
) -> OperationKind {
    match table_statement {
        StatementNode::AssemblyFact(_) => {
            unreachable!("assembly facts are filtered before operation construction")
        }
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
    program: &CheckedTrees,
    statement: &StatementNode,
    state_graph: &mut StateGraph,
) -> OperationExpressionRefs {
    match statement {
        StatementNode::AssemblyFact(_) => OperationExpressionRefs::None,
        StatementNode::Assignment(assignment) => OperationExpressionRefs::Assignment {
            target: copy_runtime_expression(state_graph, program, assignment.target),
            value: copy_runtime_expression(state_graph, program, assignment.value),
        },
        StatementNode::Call(call) => OperationExpressionRefs::Call {
            arguments: copy_statement_expression_span(state_graph, program, call.arguments),
        },
        StatementNode::Expression(expression) => OperationExpressionRefs::Expression(
            copy_runtime_expression(state_graph, program, *expression),
        ),
        StatementNode::LocalData(local_data) if local_data.initial_value.is_valid() => {
            OperationExpressionRefs::Expression(copy_runtime_expression(
                state_graph,
                program,
                local_data.initial_value,
            ))
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

/// An `Indexed` place whose index is a RUNTIME value (not a constant integer), unwrapping any
/// `Mutable` wrapper around the index. The static-assignment fast path resolves a runtime index by
/// taking the collection BASE, so an indexed write that is runtime-indexed on both sides, or nested,
/// silently no-ops through it.
fn expression_is_runtime_indexed(table: &ExpressionTable, handle: ExpressionHandle) -> bool {
    let ExpressionNode::Indexed(indexed) = table.expression(handle) else {
        return false;
    };
    let mut index = indexed.index;
    while let ExpressionNode::Borrow(inner) = table.expression(index) {
        index = inner.target;
    }
    !matches!(table.expression(index), ExpressionNode::Integer(_))
}

/// A nested runtime-indexed target whose collection is reached THROUGH an array index and whose own
/// index `i` is a runtime value -- `grid[anything][i]` (an Indexed collection) or `rows[c].data[i]`
/// (a field array of an array-of-structs element; the collection is `Member(Indexed)`). The
/// static-assignment and constant-integer fast paths cannot lower this shape: they would silently
/// NO-OP (the element never updates). Refusing the fast-path classification lets it fall to a
/// regular `Assignment`, which is recorded as a runtime-storage write and reported cleanly by the
/// storage blocker rather than vanishing. A const index (`grid[i][0]`, `rows[i].data[0]`) resolves
/// to a fixed offset and IS lowerable, and a single index over a non-indexed base (`arr[i]`,
/// `self.field[i]`) has no array index in its collection chain, so neither is refused. (Mirrors the
/// READ fence `report_nested_runtime_indexed_read` in psi-validation.)
fn target_is_nested_runtime_indexed(table: &ExpressionTable, target: ExpressionHandle) -> bool {
    // Walk EVERY level of the target's place chain (through Indexed collections, Member receivers,
    // and Mutable). A runtime index is lowerable only when it is the BASE-CLOSEST index (its
    // collection is a plain place); a runtime index sitting above another array index no-ops. This
    // catches the final-index cases (`grid[c][j]`, `rows[c].data[j]`) AND a runtime index at an
    // INNER level with a const outer index (`cube[a][b][0]`: `[b]` is runtime above `[a]`), which a
    // top-index-only check misses.
    let mut place = target;
    loop {
        match table.expression(place) {
            ExpressionNode::Indexed(indexed) => {
                if expression_is_runtime_indexed(table, place)
                    && collection_chain_reaches_index(table, indexed.collection)
                {
                    return true;
                }
                place = indexed.collection;
            }
            ExpressionNode::Member(member) => place = member.receiver,
            ExpressionNode::Borrow(inner) => place = inner.target,
            _ => return false,
        }
    }
}

/// Whether a place chain (through `Member` receivers and `Mutable`) reaches an `Indexed` node --
/// i.e. the base is itself array-indexed. (Refining this to RUNTIME-indexed-only was tried and
/// REVERTED 2026-07-07: the const-outer shape's consumers are not wired, so it ran silently
/// wrong -- see the read fence's note in psi-validation calls.rs.)
fn collection_chain_reaches_index(table: &ExpressionTable, mut place: ExpressionHandle) -> bool {
    loop {
        match table.expression(place) {
            ExpressionNode::Indexed(_) => return true,
            ExpressionNode::Member(member) => place = member.receiver,
            ExpressionNode::Borrow(inner) => place = inner.target,
            _ => return false,
        }
    }
}

fn is_static_assignment(program: &CheckedTrees, assignment: TableAssignment) -> bool {
    let table = &program.expression_table;
    // The static-assignment fast path cannot lower an indexed write that resolves a runtime index to
    // the collection base: a nested runtime column (`grid[i][j]`) or a DUAL runtime-indexed copy
    // (`a[i] = b[j]`, both sides runtime-indexed) would silently NO-OP. Refuse the fast path so they
    // fall to a regular `Assignment`, get recorded, and are reported cleanly by the storage blocker.
    // (The top-level dual case is also caught by the dual-runtime-indexed mutation blocker; this
    // additionally fences the IN-LOOP dual copy, which otherwise had no write record at all.)
    if target_is_nested_runtime_indexed(table, assignment.target)
        || (expression_is_runtime_indexed(table, assignment.target)
            && expression_is_runtime_indexed(table, assignment.value))
    {
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
