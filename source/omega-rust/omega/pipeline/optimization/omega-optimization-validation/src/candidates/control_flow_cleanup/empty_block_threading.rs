//! Linear and path-qualified empty-block threading validation.

use super::*;

pub fn validate_linear_empty_block_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::OwnershipFrontiers)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
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
        .functions
        .iter()
        .find(|function| function.machine == patch.predecessor.machine)
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
        psi_edge: incoming_edge,
        target: predecessor_target,
        bindings: incoming_bindings,
        trivial_affine_discards: incoming_discards,
    } = &predecessor_node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if !incoming_discards.is_empty()
        || !outgoing_discards.is_empty()
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

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.predecessor.machine)
        .expect("candidate function exists");
    let output_predecessor = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.predecessor.block)
        .and_then(|block| {
            block
                .nodes
                .get_mut(usize::try_from(patch.predecessor.node).ok()?)
        })
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor_edge = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let empty_edge = empty_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.outgoing_edge)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let mut combined_sources = predecessor_edge.provenance.clone();
    combined_sources.extend_from_slice(&empty_edge.provenance);
    let mut combined_fuel = predecessor_edge.fuel.clone();
    combined_fuel.extend_from_slice(&empty_edge.fuel);
    output_predecessor.operation = O::Jump {
        psi_edge: patch.incoming_edge,
        target: patch.target,
        bindings: composed_bindings,
        trivial_affine_discards: Vec::new(),
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
            node.effect = omega_optimization_unit::EffectLink {
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
        .find(|function| function.machine == patch.predecessor.machine)
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
            b"omega.validator.linear-empty-block-thread.v2",
        ),
        provenance: accepted_provenance,
    })
}

/// Independently replay an all-predecessor empty-block bypass. Every incoming
/// edge remains its own output occurrence; the removed outgoing occurrence is
/// copied only onto that mutually exclusive edge antichain.
pub fn validate_path_qualified_empty_block_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::OwnershipFrontiers)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
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
            node.effect = omega_optimization_unit::EffectLink {
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
