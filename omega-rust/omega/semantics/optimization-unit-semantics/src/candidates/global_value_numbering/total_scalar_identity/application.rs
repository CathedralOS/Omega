//! Independently apply an admitted total scalar identity.

use super::*;

pub(super) fn independently_apply_total_scalar_identity(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    patch: TotalScalarIdentityRewrite,
    node_index: usize,
    affected_blocks: &[BlockId],
    provenance: Vec<ProvenanceRewrite>,
    validator: OptimizationValidatorIdentity,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("validated candidate function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("validated candidate block exists");
    let removed = output_block.nodes.remove(node_index);
    let receiver = output_block
        .nodes
        .get_mut(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&removed.provenance);
    receiver.fuel.extend_from_slice(&removed.fuel);

    let mut effect = 0_u64;
    for block in &mut output_function.blocks {
        for (index, node) in block.nodes.iter_mut().enumerate() {
            rewrite_scalar_value_uses(&mut node.operation, patch.result, patch.replacement);
            let index = u32::try_from(index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, index);
            node.uses = expected_uses(&node.operation, block.id, index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
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
    if output.accepted_obligation_facts != input.accepted_obligation_facts {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }

    let input_function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .expect("validated input function exists");
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .expect("validated output function exists");
    for input_block in &input_function.blocks {
        if !affected_blocks.contains(&input_block.id)
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
        validator,
        provenance,
    })
}
