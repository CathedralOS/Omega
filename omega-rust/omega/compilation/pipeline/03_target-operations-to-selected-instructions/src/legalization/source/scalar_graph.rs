//! Optimizer module role: executable entrance. Projects ordered scalar blocks and explicit transfers.
use crate::legalization::scalar_graph_input;
use crate::{LegalizationError, LegalizationError as Error};
use abstract_operations::AbstractOperationPlan;
use legalized_operations::{
    LegalizedScalarBlock, LegalizedScalarFunction, LegalizedScalarParameter,
};
use optimization_unit::PsiOptimizationUnit;
use target_operations::TargetOperationPlan;
mod instruction;
mod structural_case;
mod terminator;
/// Project one function. `current_operation` records the node this projection
/// has reached, so the caller's attribution can name it on a refusal raised
/// anywhere beneath; it is cleared outside any one node.
pub(super) fn derive<'a, 'g>(
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &'a TargetOperationPlan,
    plan: &'a AbstractOperationPlan,
    unit: &'a PsiOptimizationUnit,
    current_operation: &mut Option<semantic_vocabulary::OperationId>,
    call_descriptor_origins: &mut scalar_graph_input::byte_views::CallDescriptorOrigins<'a, 'g>,
) -> Result<LegalizedScalarFunction, LegalizationError>
where
    'a: 'g,
{
    // Target lowering already chose addresses, homes and call placements.
    // Selection consumes those rows; reconstructing loan legality here both
    // duplicates Psi and loses relationships the target producer retained.
    // The independent source/target matcher is an explicit retained audit.
    let call_plan = target.graph.call_plan.clone();
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
    if optimized.blocks.len() != target.graph.blocks.len() {
        return Err(Error::custody());
    }
    let blocks = optimized
        .blocks
        .iter()
        .zip(&target.graph.blocks)
        .map(|(block, targeted)| {
            *current_operation = None;
            let (last, body) = block.nodes.split_last().ok_or(Error::custody())?;
            let rows = targeted
                .operations
                .split_at(
                    targeted
                        .operations
                        .iter()
                        .position(|row| {
                            matches!(
                                row,
                                target_operations::TargetUnitOperation::CleanupCall { .. }
                            )
                        })
                        .unwrap_or(targeted.operations.len()),
                )
                .0;
            if block.id != targeted.block || body.len() != rows.len() {
                return Err(Error::custody());
            }
            let mut instructions = Vec::with_capacity(body.len());
            for (node, row) in body.iter().zip(rows) {
                *current_operation =
                    scalar_graph_input::instruction(node).map(|(operation, _)| operation);
                instructions.push(instruction::project(
                    node,
                    optimized,
                    native,
                    plan,
                    unit,
                    row,
                    call_descriptor_origins,
                )?);
            }
            *current_operation = None;
            Ok(LegalizedScalarBlock {
                id: block.id,
                parameters: block.parameters.clone(),
                structural_parameters: block.structural_parameters.clone(),
                instructions,
                terminator: terminator::project(last, optimized, plan, native, unit)?,
            })
        })
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    Ok(LegalizedScalarFunction {
        machine: target.machine,
        attachment: target.attachment,
        nominal_cleanup_subject: target.nominal_cleanup_subject,
        provenance: target.provenance.clone(),
        call_plan,
        parameters,
        structural: scalar_graph_input::structural_contract(target, abstracted, optimized, plan),
        entry_block: optimized.entry,
        blocks,
        // The unit's verifier-owned frontier catalog reaches the access-roster
        // producer only through this projection: every fact covering this
        // function's machine is retained byte-exact so the roster binding a
        // later stage emits can name the same fact identities.
        ownership_frontier_facts: unit
            .ownership_frontier_facts
            .iter()
            .filter(|fact| fact.machine == target.machine)
            .cloned()
            .collect(),
    })
}
