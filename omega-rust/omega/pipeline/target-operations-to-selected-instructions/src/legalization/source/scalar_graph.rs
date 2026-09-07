//! Optimizer module role: executable entrance. Projects ordered scalar blocks and explicit transfers.
use super::shared::*;
use crate::legalization::scalar_graph_input;
use ::legalized_operations::*;
mod instruction;
mod terminator;
pub(super) fn derive(
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarFunction, LegalizationError> {
    let call_plan =
        scalar_graph_input::match_input(target, abstracted, optimized, native, plan, unit)?;
    let parameters = optimized
        .parameters
        .iter()
        .zip(&call_plan.parameters)
        .map(|(parameter, placement)| {
            Ok(LegalizedScalarParameter {
                value: parameter.value,
                scalar_type: scalar_graph_input::integer_type(parameter.scalar_type)
                    .ok_or(Error::SourceCustodyMismatch)?,
                definition_site: parameter.site,
                placement: placement.clone(),
            })
        })
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    let blocks = optimized
        .blocks
        .iter()
        .map(|block| {
            let (last, body) = block
                .nodes
                .split_last()
                .ok_or(Error::SourceCustodyMismatch)?;
            Ok(LegalizedScalarBlock {
                id: block.id,
                parameters: block.parameters.clone(),
                instructions: body
                    .iter()
                    .map(|node| instruction::project(node, optimized, native, plan, unit))
                    .collect::<Result<Vec<_>, LegalizationError>>()?,
                terminator: terminator::project(last)?,
            })
        })
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    Ok(LegalizedScalarFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        call_plan,
        parameters,
        entry_block: optimized.entry,
        blocks,
    })
}
