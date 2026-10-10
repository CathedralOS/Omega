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
//! final checking consumes the typed program.

use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

/// Whether any typed call still carries a retained data-layout query. The
/// query itself is already the pending marker: lowering admits the request
/// only as an explicit `DataLayoutQueryRequest`, so the field's presence is
/// the complete deferral condition — no admission or selection closure is
/// needed to know the answer is target-dependent.
pub(crate) fn pending_layout_queries_need_operator_selection(typed: &TypedTrees) -> bool {
    typed
        .expression_table
        .iter_expressions()
        .any(|(_, expression)| match expression {
            ExpressionNode::Call(call) => call.layout_query.is_some(),
            _ => false,
        })
}
