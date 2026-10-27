//! Independent path-qualified empty-block replay mechanics.
use crate::candidates::accept_region_rewrite;
use crate::candidates::control_flow_cleanup::{patched_jump_bindings, require_scalar_bindings};
use crate::candidates::rewrite_accounting::substitutions::rewrite_successor_operation;
use crate::candidates::rewrite_accounting::threading::{
    reconstruct_linear_thread_bindings, reconstruct_linear_thread_ownership_is_identity,
    reconstruct_path_thread_accounting,
};
use crate::candidates::sweep_output_nodes;
use crate::unit_validation::derived_metadata::{
    expected_definitions, expected_ownership, expected_uses,
};
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch};

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.empty) || patch.empty.node != 0 {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .function(patch.empty.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    require_scalar_bindings(function)?;

    if function.entry == patch.empty.block {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let empty_block = function
        .block(patch.empty.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [empty_node] = empty_block.nodes.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let outgoing_bindings = patched_jump_bindings(empty_node, patch.outgoing_edge, patch.target)?;
    if empty_block.parameters.iter().any(|parameter| {
        function.blocks.iter().any(|block| {
            block.nodes.iter().any(|node| {
                node.uses.iter().any(|use_site| {
                    use_site.value == parameter.value
                        && (use_site.block != empty_block.id || use_site.node != 0)
                })
            })
        })
    }) {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let mut incoming = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            for edge in node
                .successors
                .iter()
                .filter(|edge| edge.target == patch.empty.block)
            {
                if !edge.cleanup_actions.is_empty() {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                let composed = reconstruct_linear_thread_bindings(
                    &empty_block.parameters,
                    &edge.bindings,
                    outgoing_bindings,
                )
                .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
                if !reconstruct_linear_thread_ownership_is_identity(
                    input,
                    function,
                    edge.psi_edge,
                    patch.empty.block,
                    patch.outgoing_edge,
                    patch.target,
                ) {
                    return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
                }
                incoming.push((
                    NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).map_err(|_| {
                            OptimizationUnitValidationError::CandidateLocationMissing
                        })?,
                    },
                    edge.psi_edge,
                    composed,
                ));
            }
        }
    }
    if incoming.is_empty()
        || (incoming.len() == 1
            && matches!(
                function
                    .block(incoming[0].0.block)
                    .and_then(|block| block.nodes.get(usize::try_from(incoming[0].0.node).ok()?))
                    .map(|node| &node.operation),
                Some(O::Jump { .. })
            ))
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let incoming_edges = incoming
        .iter()
        .map(|(_, edge, _)| *edge)
        .collect::<Vec<_>>();
    let (expected_blocks, accepted_provenance) =
        reconstruct_path_thread_accounting(function, patch.empty, &incoming_edges)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;

    let outgoing_edge = empty_node
        .edge(patch.outgoing_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    accept_region_rewrite(
        input,
        candidate,
        patch.empty.machine,
        &expected_blocks,
        b"omega.validator.path-qualified-empty-block-thread.v1",
        accepted_provenance,
        |output| {
            let output_function = output
                .function_mut(patch.empty.machine)
                .expect("candidate function exists");
            for (location, incoming_edge, composed) in &incoming {
                let node = output_function
                    .block_mut(location.block)
                    .and_then(|block| block.nodes.get_mut(usize::try_from(location.node).ok()?))
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                if !rewrite_successor_operation(
                    &mut node.operation,
                    *incoming_edge,
                    patch.target,
                    composed,
                ) {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                let edge = node
                    .successors
                    .iter_mut()
                    .find(|edge| edge.psi_edge == *incoming_edge)
                    .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                edge.target = patch.target;
                edge.bindings = composed.clone();
                edge.provenance.extend_from_slice(&outgoing_edge.provenance);
                edge.fuel.extend_from_slice(&outgoing_edge.fuel);
                node.definitions =
                    expected_definitions(&node.operation, location.block, location.node);
                node.uses = expected_uses(&node.operation, location.block, location.node);
                node.ownership = expected_ownership(&node.operation);
            }
            output_function
                .blocks
                .retain(|block| block.id != patch.empty.block);
            sweep_output_nodes(output_function, |_| Ok(()))?;
            Ok(())
        },
    )
}
