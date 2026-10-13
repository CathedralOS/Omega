//! `size_of`/`align_of` deferral trigger.
//!
//! A retained layout query names its subject from the target closure and is
//! answered only once settlement selects the native target and the opaque
//! representation selections; the frontend evaluation domain carries no
//! target facts. Any program containing one therefore keeps its value-
//! dependent const work pending until `evaluate_selected_operators` resumes
//! under exact selected execution — the same deferral family range endpoints
//! and const applications already use — and Omega folds every retained query
//! (constant and runtime positions alike) under that target custody before
//! the target closure consumes the typed program. Only the continuation's
//! owned declaration roots participate; Product descriptions retained during
//! Build admission never become host-target evaluation work.

use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

/// Whether any typed call still carries a retained data-layout query. The
/// query itself is already the pending marker: lowering admits the request
/// only as an explicit `DataLayoutQueryRequest`, so the field's presence is
/// the complete deferral condition — no admission or selection closure is
/// needed to know the answer is target-dependent. Ownership traversal excludes
/// unselected declarations and stale arena nodes.
pub(crate) fn pending_layout_queries_need_operator_selection(
    typed: &TypedTrees,
    selection: &typed_trees::references::DeclarationSelection,
) -> bool {
    typed_trees::references::collect_selected_program_references(typed, selection)
        .expressions
        .into_iter()
        .any(
            |expression| match typed.expression_table.expression(expression) {
                ExpressionNode::Call(call) => call.layout_query.is_some(),
                _ => false,
            },
        )
}
