//! Executable-expression membership, without result metadata edges.

/// These membership sets are never iterated for authored output order.
type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;
use crate::frozen_program::expression_table;
use typed_trees::expression::ExpressionHandle;

/// Whether the containment walk from `root` reaches `target`.
pub fn expression_contains(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
    target: ExpressionHandle,
) -> bool {
    // Exact identity and leaf membership need no traversal inventory. Keep
    // the shared walk unchanged for callers collecting the complete DAG.
    if !root.is_valid() || !target.is_valid() {
        return false;
    }
    if root == target {
        return true;
    }
    if matches!(
        expression_table(program).expression(root),
        typed_trees::literal_expression_pattern!()
    ) {
        return false;
    }
    reaches(program, root, target, &mut HashSet::default())
}

/// Every expression the containment walk reaches from `root`.
///
/// `expression_contains(program, root, target)` holds exactly when this set
/// contains `target`: the walk records each expression it enters before
/// comparing it, and an address no expression occupies never ends the walk
/// early, so the recorded set is the whole reachable set. A caller asking
/// the same containment question of many targets indexes this once instead
/// of rewalking `root` per target.
pub fn reachable_expressions(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
) -> HashSet<ExpressionHandle> {
    let mut reached = HashSet::default();
    // Arena index zero is the invalid address; `is_valid` rejects it below,
    // so no recorded expression can equal it.
    reaches(program, root, ExpressionHandle::invalid(), &mut reached);
    reached
}

/// The shared walk: a membership set, never an ordered history, so an
/// expression reached twice through different parents is entered once.
fn reaches(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
    target: ExpressionHandle,
    visited: &mut HashSet<ExpressionHandle>,
) -> bool {
    program.subtree_any(root, typed_trees::SubtreeEdges::VALUES, |expression, _| {
        if !visited.insert(expression) {
            typed_trees::SubtreeVisit::Prune
        } else if expression == target {
            typed_trees::SubtreeVisit::Found
        } else {
            typed_trees::SubtreeVisit::Descend
        }
    })
}
