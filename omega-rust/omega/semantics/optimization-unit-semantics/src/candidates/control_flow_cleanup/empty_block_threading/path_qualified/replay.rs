//! Independent path-qualified empty-block replay mechanics.

use super::*;

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
        .functions
        .iter()
        .find(|function| function.machine == patch.empty.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == patch.empty.block {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let empty_block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.empty.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [empty_node] = empty_block.nodes.as_slice() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let O::Jump {
        psi_edge: outgoing_edge,
        target,
        bindings: outgoing_bindings,
        trivial_affine_discards: outgoing_discards,
    } = &empty_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if !outgoing_discards.is_empty()
        || *outgoing_edge != patch.outgoing_edge
        || *target != patch.target
    {
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
    let mut incoming = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            for edge in node
                .successors
                .iter()
                .filter(|edge| edge.target == patch.empty.block)
            {
                if !edge.trivial_affine_discards.is_empty() {
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
                    .blocks
                    .iter()
                    .find(|block| block.id == incoming[0].0.block)
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
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let outgoing_edge = empty_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.outgoing_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.empty.machine)
        .expect("candidate function exists");
    for (location, incoming_edge, composed) in &incoming {
        let node = output_function
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .and_then(|block| block.nodes.get_mut(usize::try_from(location.node).ok()?))
            .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        if !rewrite_successor_operation(&mut node.operation, *incoming_edge, patch.target, composed)
        {
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
        node.definitions = expected_definitions(&node.operation, location.block, location.node);
        node.uses = expected_uses(&node.operation, location.block, location.node);
        node.ownership = expected_ownership(&node.operation);
    }
    output_function
        .blocks
        .retain(|block| block.id != patch.empty.block);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .expect("validated function effect count fits u64"),
            };
            effect = effect
                .checked_add(1)
                .expect("validated function effect count fits u64");
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;

    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.empty.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if input_block.id != patch.empty.block
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
            b"omega.validator.path-qualified-empty-block-thread.v1",
        ),
        provenance: accepted_provenance,
    })
}
