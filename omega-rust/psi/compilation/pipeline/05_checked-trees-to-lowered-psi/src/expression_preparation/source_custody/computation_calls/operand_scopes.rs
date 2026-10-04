//! Source operand coordinates, independent of arithmetic template meaning.
//!
//! An enclosing short circuit may retain either operand's computation. Apply
//! nodes therefore name their exact authored operation, just as Select nodes
//! name their conditional. Do not infer a surviving operand from expression
//! shape: a known result can still require evaluating the left operand's calls.
use super::{CheckedTrees, ExpressionHandle, ExpressionNode};

pub(super) fn anonymous_match_value(
    checked: &CheckedTrees,
    source: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Match(dispatch) = checked.expression_table.expression(source) else {
        return None;
    };
    validation::select_anonymous_numeric_match_arm(&checked.typed, dispatch, |expression| {
        checked
            .facts
            .operators
            .expression_use(expression)
            .is_none_or(|operator_use| {
                operator_use.status
                    == checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            })
    })
}
