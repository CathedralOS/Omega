use super::super::shared::*;
use super::super::validators::ValidatedStructuralUnitForm;
use super::boundary_settlement::replay_boundary_settlement;
use super::call::replay_structural_call;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_structural_operations(
    function: usize,
    proposed: &LegalizedStructuralUnitFunction,
    validated: &ValidatedStructuralUnitForm<'_>,
    caller_claims: &[terminal_psi::EntryClaim],
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    if let Some((target_rows, abstract_rows, optimized_rows)) = validated.settlement_rows {
        for (index, (((target_row, abstract_row), optimized_row), proposed_row)) in target_rows
            .iter()
            .zip(abstract_rows)
            .zip(optimized_rows)
            .zip(&proposed.boundary_settlements)
            .enumerate()
        {
            replay_boundary_settlement(
                function,
                index,
                target_row,
                abstract_row,
                optimized_row,
                proposed_row,
                &proposed.parameters,
                caller_claims,
                abstract_plan,
            )?;
        }
    }
    match (
        validated.target_call,
        validated.abstract_call,
        validated.optimized_call,
        &proposed.call,
    ) {
        (None, None, None, None) => {}
        (Some(target_call), Some(abstract_call), Some(optimized_call), Some(proposed_call)) => {
            replay_structural_call(
                function,
                target_call,
                abstract_call,
                optimized_call,
                proposed_call,
                &proposed.parameters,
                caller_claims,
                target_plan,
                abstract_plan,
                unit,
            )?;
        }
        _ => return Err(Error::NonCanonicalLegalizedPlan),
    }
    Ok(())
}
