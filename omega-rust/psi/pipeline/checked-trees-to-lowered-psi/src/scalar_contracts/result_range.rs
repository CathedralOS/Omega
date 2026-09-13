//! A numeric result refinement is an ordinary normal-return obligation, not
//! permission to narrow the returned carrier or assume the authored body fits.

use super::*;
use checked_trees::types::TypeReferenceNode;

pub(crate) fn with_result_range(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    result_position: usize,
    plan: &ClosedScalarValueContractPlan,
) -> Result<ClosedScalarValueContractPlan, LoweringError> {
    let (_, source) = crate::scalar_source_custody::authored_state(checked, state)?;
    if !matches!(
        checked
            .type_reference_table
            .type_reference(source.return_type),
        TypeReferenceNode::Constrained { .. }
    ) {
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
    ))
}
