//! Shared terminal jump-fusion validation.

use super::*;

pub fn validate_shared_jump_fusion_candidate(
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
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::FuseSharedTerminalJump(patch) = candidate.patch() else {
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
    let predecessor = function
        .blocks
        .iter()
        .find(|block| block.id == patch.predecessor.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let predecessor_index = usize::try_from(patch.predecessor.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    if predecessor_index + 1 != predecessor.nodes.len() {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let predecessor_node = predecessor
        .nodes
        .get(predecessor_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
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
    let target = function
        .blocks
        .iter()
        .find(|block| block.id == patch.target)
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
            O::Return { .. } | O::ReturnUnit { .. } | O::ReturnStructural { .. } | O::Crash { .. }
        )
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
    if incoming.len() < 2
        || incoming
            .iter()
            .filter(|edge| edge.psi_edge == patch.incoming_edge)
            .count()
            != 1
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
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)
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
        .expect("candidate predecessor exists");
    output_predecessor.nodes[predecessor_index] = clone;
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
        if !expected_blocks.contains(&input_block.id)
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
            b"omega.validator.shared-terminal-jump-fusion.v1",
        ),
        provenance: accepted_provenance,
    })
}
