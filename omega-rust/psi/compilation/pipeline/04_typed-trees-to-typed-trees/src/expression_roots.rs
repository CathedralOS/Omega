//! The expression roots one state's own statements author, and the postorder
//! walk each consumer needs over them.

use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Append each statement's expression roots, in statement order. Contracts and
/// proof facts are not statements and stay with their own owners.
pub(crate) fn append_statement_roots(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    roots: &mut Vec<ExpressionHandle>,
) {
    for statement in program.statement_table.statements(state.statement_nodes) {
        validation::collect_statement_expression_trees(program, statement, roots);
    }
}

/// Visit roots in postorder under one visited set, so a child shared by several
/// parents is derived once, before either parent.
pub(crate) fn visit_post_order(
    program: &TypedTrees,
    roots: impl IntoIterator<Item = ExpressionHandle>,
    mut arm: impl FnMut(ExpressionHandle, &ExpressionNode),
) {
    let mut visited = Vec::new();
    for root in roots {
        validation::expression_traversal::visit_post_order(program, root, &mut visited, &mut arm);
    }
}
