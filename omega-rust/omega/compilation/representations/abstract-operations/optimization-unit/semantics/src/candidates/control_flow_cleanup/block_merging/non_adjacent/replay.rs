//! Independent non-adjacent block-merge replay mechanics.
use crate::candidates::control_flow_cleanup::{
    checked_scalar_substitutions, patched_jump_bindings, require_scalar_bindings,
};
use crate::candidates::global_value_numbering::{
    independent_reachable_dominators, independently_replacement_dominates_uses,
};
use crate::candidates::rewrite_accounting::adjacent_merge::reconstruct_adjacent_merge_ownership_is_identity;
use crate::candidates::rewrite_accounting::non_adjacent_merge::reconstruct_non_adjacent_merge_accounting;
use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
use crate::candidates::{accept_region_rewrite, candidate_function, sweep_output_nodes};
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch};

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::MergeNonAdjacentBlock(patch) = candidate.patch() else {
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
    if function.entry == patch.target
        || target_position == predecessor_position.saturating_add(1)
        || function.blocks[target_position].nodes.is_empty()
        || matches!(function.blocks[target_position].nodes.as_slice(), [node] if matches!(node.operation, O::Jump { .. }))
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let predecessor = &function.blocks[predecessor_position];
    let target = &function.blocks[target_position];
    let predecessor_index = usize::try_from(patch.predecessor.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    if predecessor_index + 1 != predecessor.nodes.len() {
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
    if !dominators
        .get(&patch.target)
        .is_some_and(|rows| rows.contains(&patch.predecessor.block))
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
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
    if !reconstruct_adjacent_merge_ownership_is_identity(
        input,
        function,
        patch.incoming_edge,
        patch.target,
    ) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_non_adjacent_merge_accounting(function, patch, &substitutions)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;

    accept_region_rewrite(
        input,
        candidate,
        patch.predecessor.machine,
        &expected_blocks,
        b"omega.validator.non-adjacent-unique-predecessor-block-merge.v1",
        accepted_provenance,
        |output| {
            let output_function = output
                .function_mut(patch.predecessor.machine)
                .expect("candidate function exists");
            let output_target_position = output_function
                .block_index(patch.target)
                .expect("candidate target exists");
            let mut moved = output_function.blocks.remove(output_target_position);
            let output_predecessor_position = output_function
                .block_index(patch.predecessor.block)
                .expect("candidate predecessor exists");
            let removed = output_function.blocks[output_predecessor_position]
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
            if !first.provenance.is_empty() {
                first.provenance.extend_from_slice(&removed_edge.provenance);
                first.fuel.extend_from_slice(&removed_edge.fuel);
            } else if !first.successors.is_empty() {
                for successor in &mut first.successors {
                    successor
                        .provenance
                        .extend_from_slice(&removed_edge.provenance);
                    successor.fuel.extend_from_slice(&removed_edge.fuel);
                }
            } else {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            output_function.blocks[output_predecessor_position]
                .nodes
                .append(&mut moved.nodes);
            sweep_output_nodes(output_function, |node| {
                for substitution in &substitutions {
                    rewrite_scalar_value_uses(
                        &mut node.operation,
                        substitution.from,
                        substitution.to,
                    );
                }
                Ok(())
            })?;
            Ok(())
        },
    )
}
