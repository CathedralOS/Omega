//! Independent linear empty-block replay mechanics.
use crate::candidates::accept_region_rewrite;
use crate::candidates::rewrite_accounting::threading::{
    reconstruct_linear_thread_accounting, reconstruct_linear_thread_bindings,
    reconstruct_linear_thread_ownership_is_identity,
};
use crate::candidates::set_node_effect_link;
use crate::unit_validation::derived_metadata::{
    expected_definitions, expected_edges, expected_ownership, expected_uses,
};
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch};

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::ThreadLinearEmptyBlock(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.predecessor)
        || patch.empty.node != 0
        || patch.empty.machine != patch.predecessor.machine
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .function(patch.predecessor.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    // These rewrites substitute scalar bindings only.
    if function
        .blocks
        .iter()
        .any(|block| !block.structural_parameters.is_empty())
    {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }

    if function.entry == patch.empty.block {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let empty_block = function
        .block(patch.empty.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [empty_node] = empty_block.nodes.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let O::Jump {
        structural_bindings: _,
        psi_edge: outgoing_edge,
        target,
        bindings: outgoing_bindings,
        cleanup_actions: outgoing_cleanup_actions,
    } = &empty_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if *outgoing_edge != patch.outgoing_edge || *target != patch.target {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
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

    let incoming = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .filter_map(move |(node_index, node)| {
                    node.successors
                        .iter()
                        .any(|edge| edge.target == patch.empty.block)
                        .then_some((block, node_index, node))
                })
        })
        .collect::<Vec<_>>();
    let [(predecessor_block, predecessor_index, predecessor_node)] = incoming.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    };
    let predecessor_location = NodeLocation {
        machine: function.machine,
        block: predecessor_block.id,
        node: u32::try_from(*predecessor_index)
            .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
    };
    let O::Jump {
        structural_bindings: _,
        psi_edge: incoming_edge,
        target: predecessor_target,
        bindings: incoming_bindings,
        cleanup_actions: incoming_cleanup_actions,
    } = &predecessor_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if !incoming_cleanup_actions.is_empty()
        || !outgoing_cleanup_actions.is_empty()
        || predecessor_location != patch.predecessor
        || *incoming_edge != patch.incoming_edge
        || *predecessor_target != patch.empty.block
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let composed_bindings = reconstruct_linear_thread_bindings(
        &empty_block.parameters,
        incoming_bindings,
        outgoing_bindings,
    )
    .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
    if !reconstruct_linear_thread_ownership_is_identity(
        input,
        function,
        patch.incoming_edge,
        patch.empty.block,
        patch.outgoing_edge,
        patch.target,
    ) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_linear_thread_accounting(function, patch.predecessor, patch.empty)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if candidate.provenance().len() != accepted_provenance.len()
        || candidate
            .provenance()
            .iter()
            .zip(&accepted_provenance)
            .any(|(actual, expected)| {
                actual.input != expected.input
                    || actual.disposition != expected.disposition
                    || actual.sources != expected.sources
            })
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if candidate
        .provenance()
        .iter()
        .zip(&accepted_provenance)
        .any(|(actual, expected)| actual.fuel != expected.fuel)
    {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }

    accept_region_rewrite(
        input,
        candidate,
        patch.predecessor.machine,
        &expected_blocks,
        b"omega.validator.linear-empty-block-thread.v2",
        accepted_provenance,
        |output| {
            let output_function = output
                .function_mut(patch.predecessor.machine)
                .expect("candidate function exists");
            let output_predecessor = output_function
                .block_mut(patch.predecessor.block)
                .and_then(|block| {
                    block
                        .nodes
                        .get_mut(usize::try_from(patch.predecessor.node).ok()?)
                })
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let predecessor_edge = predecessor_node
                .edge(patch.incoming_edge)
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            let empty_edge = empty_node
                .edge(patch.outgoing_edge)
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            let mut combined_sources = predecessor_edge.provenance.clone();
            combined_sources.extend_from_slice(&empty_edge.provenance);
            let mut combined_fuel = predecessor_edge.fuel.clone();
            combined_fuel.extend_from_slice(&empty_edge.fuel);
            output_predecessor.operation = O::Jump {
                structural_bindings: Vec::new(),
                psi_edge: patch.incoming_edge,
                target: patch.target,
                bindings: composed_bindings,
                cleanup_actions: Vec::new(),
            };
            output_predecessor.definitions = expected_definitions(
                &output_predecessor.operation,
                patch.predecessor.block,
                patch.predecessor.node,
            );
            output_predecessor.uses = expected_uses(
                &output_predecessor.operation,
                patch.predecessor.block,
                patch.predecessor.node,
            );
            output_predecessor.successors = expected_edges(&output_predecessor.operation);
            output_predecessor.successors[0].provenance = combined_sources;
            output_predecessor.successors[0].fuel = combined_fuel;
            output_predecessor.ownership = expected_ownership(&output_predecessor.operation);
            output_predecessor.provenance.clear();
            output_predecessor.fuel.clear();
            output_function
                .blocks
                .retain(|block| block.id != patch.empty.block);
            let mut effect = 0u64;
            for block in &mut output_function.blocks {
                for node in &mut block.nodes {
                    effect = set_node_effect_link(node, effect)?;
                }
            }
            Ok(())
        },
    )
}
