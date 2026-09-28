//! Optimizer module role: executable entrance. Independently joins all graph rows to source CFG and ABI.
use crate::legalization::scalar_graph_input;
use crate::{LegalizationError, LegalizationError as Error};
use abstract_operations::AbstractOperationPlan;
use legalized_operations::{LegalizedOperationPlan, LegalizedScalarFunction};
use optimization_unit::PsiOptimizationUnit;
use target_operations::TargetOperationPlan;
mod instruction;
mod structural_case;
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
    if proposed.structural
        != scalar_graph_input::structural_contract(target, abstracted, optimized, plan)
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    let invalid = Error::NonCanonicalLegalizedPlan;
    // The frontier catalog is re-projected from the source unit, never read
    // back from the proposal: replay establishes the same machine-scoped rows
    // independently, so a forged or dropped fact identity fails the join.
    let frontier_facts: Vec<_> = unit
        .ownership_frontier_facts
        .iter()
        .filter(|fact| fact.machine == target.machine)
        .cloned()
        .collect();
    if proposed.machine != target.machine
        || proposed.attachment != target.attachment
        || proposed.provenance != target.provenance
        || proposed.call_plan != call_plan
        || proposed.entry_block != optimized.entry
        || proposed.parameters.len() != optimized.parameters.len()
        || proposed.blocks.len() != optimized.blocks.len()
        || proposed.ownership_frontier_facts != frontier_facts
        || proposed
            .parameters
            .iter()
            .zip(&optimized.parameters)
            .zip(&call_plan.parameters)
            .any(|((actual, source), placement)| {
                actual.value != source.value
                    || actual.scalar_type != source.scalar_type
                    || actual.definition_site != source.site
                    || actual.placement != *placement
            })
    {
        return Err(invalid);
    }
    // Replay reference custody block by block; each row's structural
    // arguments rejoin against the custody state its position suspends.
    let custody_entries =
        scalar_graph_input::reference_custody::block_entry_states(optimized, plan, unit)?;
    for (block, source) in proposed.blocks.iter().zip(&optimized.blocks) {
        let (last, body) = source.nodes.split_last().ok_or(invalid.clone())?;
        if block.id != source.id
            || block.parameters != source.parameters
            || block.structural_parameters != source.structural_parameters
        {
            return Err(invalid);
        }
        let mut custody = custody_entries
            .get(&block.id)
            .cloned()
            .ok_or(invalid.clone())?;
        // Descriptor-parameter declarations retain no operation identity and
        // so have no instruction row; a fabricated payload on one stays in
        // `body` and fails the pairing below.
        let body: Vec<_> = body
            .iter()
            .filter(|node| !scalar_graph_input::indirect_calls::is_descriptor_declaration(node))
            .collect();
        if block.instructions.len() != body.len() {
            return Err(invalid);
        }
        for (actual, node) in block.instructions.iter().zip(&body) {
            instruction::validate(
                actual,
                node,
                optimized,
                native,
                plan,
                unit,
                proposed_plan,
                &custody,
            )?;
            scalar_graph_input::reference_custody::apply(
                &mut custody,
                &node.operation,
                optimized,
                plan,
                unit,
            )?;
        }
        // Edge-local discards move custody before the terminator's return
        // roster is rejoined, mirroring the lowering's cleanup-first order.
        scalar_graph_input::reference_custody::apply_terminator(
            &mut custody,
            &last.operation,
            optimized,
            plan,
        )?;
        terminator::validate(&block.terminator, last, optimized, plan, &custody)?;
    }
    Ok(())
}
