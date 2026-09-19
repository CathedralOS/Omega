//! A numeric result refinement is an ordinary normal-return obligation, not
//! permission to narrow the returned carrier or assume the authored body fits.
use super::super::{CheckedTrees, ClosedScalarValueContractPlan};
use super::{
    CheckedBooleanExpression, CheckedScalarExpression, ClosedScalarContractValue, LoweringError,
};
use checked_trees::types::TypeReferenceNode;

pub(crate) fn with_result_range(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    result_position: usize,
    plan: &ClosedScalarValueContractPlan,
) -> Result<ClosedScalarValueContractPlan, LoweringError> {
    let (_, source) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    if !matches!(
        checked
            .type_reference_table
            .type_reference(source.return_type),
        TypeReferenceNode::Constrained { .. }
    ) || validation::is_arithmetic_policy_only_integer(&checked.typed, source.return_type)
    {
        return Ok(plan.clone());
    }
    // This query describes the exact authored interval only. The guarantee
    // below still has to be proved independently at every actual normal exit.
    let (primitive_type, minimum, maximum) =
        validation::closed_scalar_result_range(&checked.typed, source.return_type).ok_or(
            LoweringError::Unsupported("scalar result requires one closed exact integer range"),
        )?;
    let subject = CheckedScalarExpression::Parameter {
        position: result_position,
        primitive_type,
    };
    let predicate = CheckedBooleanExpression::And {
        left: Box::new(CheckedBooleanExpression::IntegerComparison {
            kind: checked_trees::CheckedIntegerComparisonKind::LessOrEqual,
            left: Box::new(CheckedScalarExpression::IntegerLiteral { literal: minimum }),
            right: Box::new(subject.clone()),
        }),
        right: Box::new(CheckedBooleanExpression::IntegerComparison {
            kind: checked_trees::CheckedIntegerComparisonKind::LessOrEqual,
            left: Box::new(subject),
            right: Box::new(CheckedScalarExpression::IntegerLiteral { literal: maximum }),
        }),
    };
    let mut ensures = plan.ensures().to_vec();
    ensures.push(Some(ClosedScalarContractValue::Predicate(predicate)));
    Ok(ClosedScalarValueContractPlan::new(
        plan.requires().to_vec(),
        ensures,
        plan.has_crash_clauses(),
        plan.has_outcome_specific_clauses(),
    )
    // The rebuilt plan keeps the authored/deduced requires split verbatim so
    // derived range rows stay out of the published authored clause.
    .with_authored_requires_len(plan.authored_requires().len())
    // A result refinement cannot reconstruct the retained floating entry
    // roster; it rides back through unchanged so requires-tail `FloatRange`
    // clauses keep their delivery evidence.
    .with_float_entry_ranges(plan.float_entry_ranges().map(<[_]>::to_vec)))
}
