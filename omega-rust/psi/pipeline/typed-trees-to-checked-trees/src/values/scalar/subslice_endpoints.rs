//! The present endpoints of one authored exclusive `view[start..end]` range.
//!
//! A call argument, a transition argument and a `let` initializer narrow a
//! view with the same builtin range, so every site lowers its endpoints here
//! under the shared `SubsliceStart`/`SubsliceEnd` roles keyed by its
//! `CheckedSubsliceSite`. An omitted endpoint produces no row: zero and the
//! source length are supplied by the range's consumer, not by a scalar fact.
use checked_trees::{
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedSubsliceSite,
};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Lower the present endpoints of `expression` when it is an exclusive range
/// whose operator use (if any) resolved to the builtin meaning. `lower`
/// evaluates one endpoint as a `u64` in the site's own scalar namespace.
pub(crate) fn subslice_endpoints(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    site: CheckedSubsliceSite,
    mut lower: impl FnMut(ExpressionHandle) -> Option<CheckedScalarExpression>,
) -> Vec<(
    ExpressionHandle,
    CheckedScalarExpressionRole,
    CheckedScalarExpression,
)> {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return Vec::new();
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return Vec::new();
    };
    if range.end_inclusive
        || operators
            .expression_use(expression)
            .is_some_and(|selected| {
                selected.spelling != language_core::OperatorSpelling::Range
                    || selected.selected_operator_symbol.is_valid()
                    || selected.candidate_count != 0
                    || !matches!(
                        selected.status,
                        CheckedOperatorResolutionStatus::Missing
                            | CheckedOperatorResolutionStatus::BuiltinFallback
                    )
            })
    {
        return Vec::new();
    }
    [
        (
            range.start,
            CheckedScalarExpressionRole::SubsliceStart { site },
        ),
        (range.end, CheckedScalarExpressionRole::SubsliceEnd { site }),
    ]
    .into_iter()
    .filter(|(endpoint, _)| endpoint.is_valid())
    .filter_map(|(endpoint, role)| Some((endpoint, role, lower(endpoint)?)))
    .collect()
}
