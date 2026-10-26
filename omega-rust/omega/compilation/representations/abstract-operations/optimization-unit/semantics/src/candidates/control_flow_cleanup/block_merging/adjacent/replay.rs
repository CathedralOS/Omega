//! Independent adjacent block-merge replay mechanics.
use crate::candidates::control_flow_cleanup::{
    checked_scalar_substitutions, patched_jump_bindings, require_scalar_bindings,
};
use crate::candidates::global_value_numbering::{
    independent_reachable_dominators, independently_replacement_dominates_uses,
};
use crate::candidates::rewrite_accounting::adjacent_merge::{
    reconstruct_adjacent_merge_accounting, reconstruct_adjacent_merge_ownership_witness,
};
use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
use crate::candidates::{
    accept_region_rewrite, candidate_function, rebuild_node_metadata, set_node_effect_link,
};
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{PsiOptimizationUnit, PsiProvenance, PsiRewriteCandidate, PsiRewritePatch};

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::MergeAdjacentBlock(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.predecessor) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = candidate_function(input, patch.predecessor.machine)?;
    require_scalar_bindings(function)?;

    let predecessor_position = function
        .block_index(patch.predecessor.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let target_position = function
        .block_index(patch.target)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if target_position != predecessor_position + 1 || function.entry == patch.target {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let predecessor = &function.blocks[predecessor_position];
    let target = &function.blocks[target_position];
    let predecessor_index = usize::try_from(patch.predecessor.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let eligible_first = target.nodes.first().is_some_and(|node| {
        (node.successors.is_empty()
            && (matches!(node.provenance.first(), Some(PsiProvenance::Operation(_)))
                || (matches!(node.provenance.first(), Some(PsiProvenance::Edge(_)))
                    && matches!(
                        node.operation,
                        abstract_operations::return_or_return_unit_pattern!()
                            | abstract_operations::return_structural_or_crash_pattern!()
                    ))))
            || (matches!(node.operation, O::Conditional { .. })
                && node.successors.len() == 2
                && node.provenance.is_empty())
    });
    if predecessor_index + 1 != predecessor.nodes.len() || !eligible_first {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let predecessor_node = &predecessor.nodes[predecessor_index];
    let bindings = patched_jump_bindings(predecessor_node, patch.incoming_edge, patch.target)?;
    let incoming = function
        .block_edges()
        .filter(|edge| edge.target == patch.target)
        .collect::<Vec<_>>();
    if incoming.len() != 1 || incoming[0].psi_edge != patch.incoming_edge {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let dominators = independent_reachable_dominators(function);
    let substitutions = checked_scalar_substitutions(
        candidate,
        &target.parameters,
        bindings,
        |parameter, binding| {
            independently_replacement_dominates_uses(
                function,
                &dominators,
                binding.argument,
                parameter.value,
                parameter.scalar_type,
            )
        },
    )?;
    let ownership_witness = reconstruct_adjacent_merge_ownership_witness(
        input,
        function,
        patch.incoming_edge,
        patch.target,
    )
    .ok_or(OptimizationUnitValidationError::CandidateObservationMismatch)?;
    if candidate.ownership_frontier_witness() != Some(&ownership_witness) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_adjacent_merge_accounting(function, patch, &substitutions)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    accept_region_rewrite(
        input,
        candidate,
        patch.predecessor.machine,
        &expected_blocks,
        b"omega.validator.adjacent-single-predecessor-block-merge.v5",
        accepted_provenance,
        |output| {
            let output_function = output
                .function_mut(patch.predecessor.machine)
                .expect("candidate function exists");
            let mut moved = output_function.blocks.remove(target_position);
            let output_predecessor = &mut output_function.blocks[predecessor_position];
            let removed = output_predecessor
                .nodes
                .pop()
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let removed_edge = removed
                .edge(patch.incoming_edge)
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            let first = moved
                .nodes
                .first_mut()
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            if first.successors.is_empty() {
                first.provenance.extend_from_slice(&removed_edge.provenance);
                first.fuel.extend_from_slice(&removed_edge.fuel);
            } else {
                for successor in &mut first.successors {
                    successor
                        .provenance
                        .extend_from_slice(&removed_edge.provenance);
                    successor.fuel.extend_from_slice(&removed_edge.fuel);
                }
            }
            output_predecessor.nodes.append(&mut moved.nodes);
            for block in &mut output_function.blocks {
                for (node_index, node) in block.nodes.iter_mut().enumerate() {
                    for substitution in &substitutions {
                        rewrite_scalar_value_uses(
                            &mut node.operation,
                            substitution.from,
                            substitution.to,
                        );
                    }
                    rebuild_node_metadata(node, block.id, node_index)?;
                }
            }
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
