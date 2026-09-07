//! Optimizer module role: executable entrance. Independently joins all graph rows to source CFG and ABI.
use super::shared::*;
use crate::legalization::scalar_graph_input;
use ::legalized_operations::*;
mod instruction;
mod terminator;
#[allow(clippy::too_many_arguments)]
pub(super) fn replay(
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed_plan: &LegalizedOperationPlan,
    proposed: &LegalizedScalarFunction,
) -> Result<(), LegalizationError> {
    let call_plan =
        scalar_graph_input::match_input(target, abstracted, optimized, native, plan, unit)?;
    let invalid = Error::NonCanonicalLegalizedPlan;
    if proposed.machine != target.machine
        || proposed.attachment != target.attachment
        || proposed.provenance != target.provenance
        || proposed.call_plan != call_plan
        || proposed.entry_block != optimized.entry
        || proposed.parameters.len() != optimized.parameters.len()
        || proposed.blocks.len() != optimized.blocks.len()
        || proposed
            .parameters
            .iter()
            .zip(&optimized.parameters)
            .zip(&call_plan.parameters)
            .any(|((actual, source), placement)| {
                actual.value != source.value
                    || ScalarType::Integer(actual.scalar_type) != source.scalar_type
                    || actual.definition_site != source.site
                    || actual.placement != *placement
            })
    {
        return Err(invalid);
    }
    for (block, source) in proposed.blocks.iter().zip(&optimized.blocks) {
        let (last, body) = source.nodes.split_last().ok_or(invalid.clone())?;
        if block.id != source.id
            || block.parameters != source.parameters
            || block.instructions.len() != body.len()
        {
            return Err(invalid);
        }
        for (actual, node) in block.instructions.iter().zip(body) {
            instruction::validate(actual, node, optimized, native, plan, unit, proposed_plan)?;
        }
        terminator::validate(&block.terminator, last)?;
    }
    Ok(())
}
