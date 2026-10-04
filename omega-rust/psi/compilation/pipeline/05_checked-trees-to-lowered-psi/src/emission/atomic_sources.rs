//! Source correspondence for one checked atomic event.
//!
//! The checked plan names an event, its place, its orderings, its operand
//! rows and its result binding. None of that is trusted here: each fact is
//! reread from the authored carrier through the shared decoder
//! (`validation::atomic_load_carrier` / `atomic_assignment_carrier`) and the
//! place through the same projected-receiver resolution a primitive store
//! replays, so a plan that names another field, another operation, another
//! ordering, a substituted operand or a different result local refuses.
//!
//! The writing carriers occupy two statements: the result placeholder local
//! and the carrier assignment. The placeholder plans nothing; the event's
//! coordinate is the carrier and its result binding is the placeholder's.

use checked_trees::CheckedTrees;
use checked_trees::statement::StatementNode;

/// Whether the statement at `index` is an atomic writing carrier's result
/// placeholder: an immutable local immediately followed by the carrier whose
/// generated result names it. It declares the prior's binding and plans no
/// operation of its own.
pub(crate) fn result_placeholder(
    checked: &CheckedTrees,
    statements: &[StatementNode],
    index: usize,
) -> bool {
    let (Some(StatementNode::LocalData(local)), Some(StatementNode::Assignment(assignment))) =
        (statements.get(index), statements.get(index + 1))
    else {
        return false;
    };
    !local.is_mutable
        && validation::atomic_assignment_carrier(checked, assignment)
            .is_some_and(|carrier| names_local(checked, carrier.result, local))
}

fn names_local(
    checked: &CheckedTrees,
    result: checked_trees::expression::ExpressionHandle,
    local: &checked_trees::statement::TableLocalData,
) -> bool {
    if !checked.expression_table.expression_is_valid(result) {
        return false;
    }
    let checked_trees::expression::ExpressionNode::Name(path) =
        checked.expression_table.expression(result)
    else {
        return false;
    };
    if path.symbol.is_valid() {
        return path.symbol == local.symbol;
    }
    matches!(
        checked.expression_table.name_path_members(path.members),
        [member] if *member == local.name
    )
}
