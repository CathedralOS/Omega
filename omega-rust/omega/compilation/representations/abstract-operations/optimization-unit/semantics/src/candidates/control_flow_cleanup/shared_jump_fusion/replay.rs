//! Independent shared terminal-jump fusion replay mechanics.
use crate::candidates::accept_region_rewrite;
use crate::candidates::rewrite_accounting::adjacent_merge::reconstruct_adjacent_merge_ownership_is_identity;
use crate::candidates::rewrite_accounting::rewrite_scalar_substitutions;
use crate::candidates::rewrite_accounting::terminal_fusion::reconstruct_shared_terminal_fusion_accounting;
use crate::unit_validation::derived_metadata::{
    expected_definitions, expected_edges, expected_ownership, expected_uses,
};
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, PsiRewriteCandidate,
    PsiRewritePatch, ScalarSubstitution,
};
use semantic_vocabulary::BlockId;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::FuseSharedTerminalJump(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.predecessor) {
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

    let predecessor = function
        .block(patch.predecessor.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor_index = usize::try_from(patch.predecessor.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    if predecessor_index + 1 != predecessor.nodes.len() {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let predecessor_node = crate::candidates::candidate_node(predecessor, patch.predecessor.node)?;
    let O::Jump {
        structural_bindings: _,
        psi_edge,
        target: jump_target,
        bindings,
        cleanup_actions,
    } = &predecessor_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if !cleanup_actions.is_empty()
        || *psi_edge != patch.incoming_edge
        || *jump_target != patch.target
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let target = function
        .block(patch.target)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [terminal] = target.nodes.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if target.id == function.entry
        || predecessor.id == target.id
        || !terminal.successors.is_empty()
        || !matches!(terminal.provenance.first(), Some(PsiProvenance::Edge(_)))
        || !matches!(
            terminal.operation,
            abstract_operations::return_or_return_unit_pattern!()
                | abstract_operations::return_structural_or_crash_pattern!()
        )
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let incoming = function
        .block_edges()
        .filter(|edge| edge.target == patch.target)
        .collect::<Vec<_>>();
    if incoming.len() < 2
        || incoming
            .iter()
            .filter(|edge| edge.psi_edge == patch.incoming_edge)
            .count()
            != 1
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if !independently_post_dominates(function, patch.predecessor.block, patch.target) {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if target.parameters.len() != bindings.len() {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let mut substitutions = target
        .parameters
        .iter()
        .zip(bindings)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some(ScalarSubstitution {
                    from: parameter.value,
                    to: binding.argument,
                    scalar_type: parameter.scalar_type,
                })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
    substitutions.sort();
    if candidate.substitutions() != substitutions {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }
    if !reconstruct_adjacent_merge_ownership_is_identity(
        input,
        function,
        patch.incoming_edge,
        patch.target,
    ) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_shared_terminal_fusion_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let incoming_edge = predecessor_node
        .edge(patch.incoming_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?
        .clone();
    let removed_effect = predecessor_node.effect;
    let mut clone = terminal.clone();
    rewrite_scalar_substitutions(
        &mut clone.operation,
        &substitutions,
        patch.predecessor.machine,
        patch.target,
    );
    clone
        .provenance
        .extend_from_slice(&incoming_edge.provenance);
    clone.fuel.extend_from_slice(&incoming_edge.fuel);
    clone.effect = removed_effect;
    clone.definitions = expected_definitions(
        &clone.operation,
        patch.predecessor.block,
        patch.predecessor.node,
    );
    clone.uses = expected_uses(
        &clone.operation,
        patch.predecessor.block,
        patch.predecessor.node,
    );
    clone.successors = expected_edges(&clone.operation);
    clone.ownership = expected_ownership(&clone.operation);

    accept_region_rewrite(
        input,
        candidate,
        patch.predecessor.machine,
        &expected_blocks,
        b"omega.validator.shared-terminal-jump-fusion.v2",
        accepted_provenance,
        |output| {
            let output_function = output
                .function_mut(patch.predecessor.machine)
                .expect("candidate function exists");
            let output_predecessor = output_function
                .block_mut(patch.predecessor.block)
                .expect("candidate predecessor exists");
            output_predecessor.nodes[predecessor_index] = clone;
            Ok(())
        },
    )
}

fn independently_post_dominates(
    function: &PsiOptimizationFunction,
    block: BlockId,
    candidate: BlockId,
) -> bool {
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            let mut targets = block
                .nodes
                .last()
                .into_iter()
                .flat_map(|node| expected_edges(&node.operation))
                .map(|edge| edge.target)
                .collect::<Vec<_>>();
            targets.sort_unstable();
            targets.dedup();
            (block.id, targets)
        })
        .collect::<BTreeMap<_, _>>();
    let all = successors.keys().copied().collect::<BTreeSet<_>>();
    let exits = successors
        .iter()
        .filter_map(|(block, targets)| targets.is_empty().then_some(*block))
        .collect::<BTreeSet<_>>();
    if exits.is_empty() {
        return false;
    }
    let mut post_dominators = all
        .iter()
        .copied()
        .map(|block| {
            let initial = if exits.contains(&block) {
                BTreeSet::from([block])
            } else {
                all.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for current in all
            .iter()
            .copied()
            .filter(|current| !exits.contains(current))
        {
            let mut adjacent = successors[&current]
                .iter()
                .filter_map(|successor| post_dominators.get(successor));
            let Some(mut next) = adjacent.next().cloned() else {
                return false;
            };
            for set in adjacent {
                next = next.intersection(set).copied().collect();
            }
            next.insert(current);
            if post_dominators[&current] != next {
                post_dominators.insert(current, next);
                changed = true;
            }
        }
        if !changed {
            return post_dominators
                .get(&block)
                .is_some_and(|blocks| blocks.contains(&candidate));
        }
    }
}
