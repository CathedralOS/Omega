//! Optimizer module role: executable entrance. Projects ordered scalar blocks and explicit transfers.
use super::shared::*;
use crate::legalization::scalar_graph_input;
use ::legalized_operations::*;
mod instruction;
mod structural_case;
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
                scalar_type: parameter.scalar_type,
                definition_site: parameter.site,
                placement: placement.clone(),
            })
        })
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    // Reference custody is replayed block by block so `.., Referent` call
    // arguments rejoin through the exact state their position suspends. The
    // input match above already proved the same schedule; this reconstruction
    // supplies only the argument-resolution snapshot.
    let custody_entries =
        scalar_graph_input::reference_custody::block_entry_states(optimized, plan, unit)?;
    let blocks = optimized
        .blocks
        .iter()
        .map(|block| {
            let (last, body) = block
                .nodes
                .split_last()
                .ok_or(Error::SourceCustodyMismatch)?;
            let mut custody = custody_entries
                .get(&block.id)
                .cloned()
                .ok_or(Error::SourceCustodyMismatch)?;
            let mut instructions = Vec::with_capacity(body.len());
            for node in body {
                instructions.push(instruction::project(
                    node, optimized, native, plan, unit, &custody,
                )?);
                scalar_graph_input::reference_custody::apply(
                    &mut custody,
                    &node.operation,
                    optimized,
                    plan,
                    unit,
                )?;
            }
            Ok(LegalizedScalarBlock {
                id: block.id,
                parameters: block.parameters.clone(),
                structural_parameters: block.structural_parameters.clone(),
                instructions,
                terminator: terminator::project(last, optimized, plan)?,
            })
        })
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    Ok(LegalizedScalarFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        call_plan,
        parameters,
        structural: scalar_graph_input::structural_contract(target, abstracted, optimized, plan),
        entry_block: optimized.entry,
        blocks,
    })
}
