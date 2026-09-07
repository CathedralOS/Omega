use super::super::shared::*;
use super::super::validators::ValidatedStructuralUnitForm;
use super::boundary_settlement::replay_boundary_settlement;
use super::call::replay_structural_call;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_structural_operations(
    function: usize,
    proposed: &legalized_operations::LegalizedScalarFunction,
    validated: &ValidatedStructuralUnitForm<'_>,
    caller_claims: &[terminal_psi::EntryClaim],
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let signature = proposed
        .structural
        .as_ref()
        .ok_or(Error::NonCanonicalLegalizedPlan)?;
    let [block] = proposed.blocks.as_slice() else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    if let Some((target_rows, abstract_rows, optimized_rows)) = validated.settlement_rows {
        if block.instructions.len() != target_rows.len() {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
        for (index, (((target_row, abstract_row), optimized_row), row)) in target_rows
            .iter()
            .zip(abstract_rows)
            .zip(optimized_rows)
            .zip(&block.instructions)
            .enumerate()
        {
            let legalized_operations::LegalizedScalarInstructionKind::BoundarySettlement(
                settlement,
            ) = &row.kind
            else {
                return Err(Error::NonCanonicalLegalizedPlan);
            };
            if row.result.is_some()
                || row.operation != settlement.operation
                || row.fuel != settlement.fuel
                || row.effect != settlement.effect
                || row.ownership != settlement.ownership
            {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
            replay_boundary_settlement(
                function,
                index,
                target_row,
                abstract_row,
                optimized_row,
                settlement,
                &signature.parameters,
                caller_claims,
                abstract_plan,
            )?;
        }
    } else {
        match (
            validated.target_call,
            validated.abstract_call,
            validated.optimized_call,
            block.instructions.as_slice(),
        ) {
            (None, None, None, []) => {}
            (Some(target), Some(abstracted), Some(optimized), [call]) => replay_structural_call(
                function,
                target,
                abstracted,
                optimized,
                call,
                &signature.parameters,
                caller_claims,
                target_plan,
                abstract_plan,
                unit,
            )?,
            _ => return Err(Error::NonCanonicalLegalizedPlan),
        }
    }
    Ok(())
}
