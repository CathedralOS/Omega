//! Retained target observations in the current source work domain.
//!
//! Layout queries keep their original operands while source checking has no
//! target facts. Only queries belonging to this continuation can defer its
//! value-dependent work; their answers require an owning portable recipe and
//! later target geometry, never a fabricated source constant.

use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

/// A projected continuation owns only its retained source work. Queries from
/// another completed projection cannot veto this continuation's constants.
pub(crate) fn pending_layout_queries_in(
    typed: &TypedTrees,
    domain: &crate::build_time_evaluation::EvaluationDomain,
) -> bool {
    domain.references(typed).expressions.into_iter().any(|expression| {
        matches!(typed.expression_table.expression(expression),
            ExpressionNode::Call(call) if call.layout_query.is_some())
    })
}
