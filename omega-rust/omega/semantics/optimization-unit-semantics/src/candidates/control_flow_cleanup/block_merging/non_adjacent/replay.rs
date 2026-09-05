//! Independent non-adjacent block-merge replay mechanics.

use super::*;

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
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)
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
    let O::Jump {
        psi_edge,
        target: jump_target,
        bindings,
        trivial_affine_discards,
    } = &predecessor_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if !trivial_affine_discards.is_empty()
        || *psi_edge != patch.incoming_edge
        || *jump_target != patch.target
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let incoming = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
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
    if target.parameters.len() != bindings.len() {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let mut substitutions = target
        .parameters
        .iter()
        .zip(bindings)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value
                && binding.scalar_type == parameter.scalar_type
                && independently_replacement_dominates_uses(
                    function,
                    &dominators,
                    binding.argument,
                    parameter.value,
                    parameter.scalar_type,
                ))
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
        reconstruct_non_adjacent_merge_accounting(function, patch, &substitutions)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("candidate function exists");
    let output_target_position = output_function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)
        .expect("candidate target exists");
    let mut moved = output_function.blocks.remove(output_target_position);
    let output_predecessor_position = output_function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)
        .expect("candidate predecessor exists");
    let removed = output_function.blocks[output_predecessor_position]
        .nodes
        .pop()
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let removed_edge = removed
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)
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
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            for substitution in &substitutions {
                rewrite_scalar_value_uses(&mut node.operation, substitution.from, substitution.to);
            }
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
        }
    }
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = effect
                .checked_add(1)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if input_block.id != patch.target
            && !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.non-adjacent-unique-predecessor-block-merge.v1",
        ),
        provenance: accepted_provenance,
    })
}
