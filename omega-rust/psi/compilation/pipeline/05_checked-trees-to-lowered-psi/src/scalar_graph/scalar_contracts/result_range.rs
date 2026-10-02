//! A numeric result refinement is an ordinary normal-return obligation, not
//! permission to narrow the returned carrier or assume the authored body fits.
use super::super::{CheckedTrees, ClosedScalarValueContractPlan};
use super::{CheckedScalarExpression, ClosedScalarContractValue, LoweringError};
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
    ) {
        return Ok(plan.clone());
    }
    // The catalog validates semantic atoms and policy shells independently.
    // Every value refinement below adds its own normal-return predicate;
    // combining one with a tag or policy cannot discard either owner.
    let (primitive_type, _) =
        crate::expression_preparation::qualifications::type_atoms(checked, source.return_type)?;
    let subject = CheckedScalarExpression::Parameter {
        position: result_position,
        primitive_type,
    };
    let predicates =
        validation::scalar_type_refinement_predicates(&checked.typed, source.return_type, subject)
            .ok_or(LoweringError::Unsupported(
                "scalar result refinement has no complete predicate meaning",
            ))?;
    let mut ensures = plan.ensures().to_vec();
    ensures.extend(
        predicates
            .into_iter()
            .map(|predicate| Some(ClosedScalarContractValue::Predicate(predicate))),
    );
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
    .with_float_entry_ranges(plan.float_entry_ranges().map(<[_]>::to_vec))
    // The retained integer roster rides through unchanged for the same
    // reason: its rows are the exact endpoint evidence behind the
    // requires-tail `Predicate` conjunctions.
    .with_integer_entry_ranges(plan.integer_entry_ranges().map(<[_]>::to_vec)))
}
