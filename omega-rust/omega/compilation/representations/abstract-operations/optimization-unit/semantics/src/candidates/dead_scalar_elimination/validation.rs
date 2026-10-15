//! Independent dead-node rewrite reconstruction and application.
use crate::candidates::outside_region::validate_outside_region;
use crate::candidates::rewrite_accounting::dead_scalar::reconstruct_dead_scalar_node_accounting;
use crate::{
    OptimizationUnitValidationError, ValidatedPsiRewrite, reconstruct_closed_scalar_node_boundary,
};
use optimization_unit::{
    OptimizationFact, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch, ValueDefinition,
    ValueDefinitionSite,
};

use super::operation_partition::independently_validated_dead_scalar_shape;
use crate::candidates::{
    candidate_block, candidate_function, rebuild_node_metadata, set_node_effect_link,
};

pub(super) fn validate_dead_scalar_node_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::RemoveDeadScalarNode(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location)
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = candidate_function(input, patch.location.machine)?;
    let block = candidate_block(function, patch.location.block)?;
    let node_index = usize::try_from(patch.location.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = crate::candidates::candidate_node(block, patch.location.node)?;
    let (source_operation, result, scalar_type, obligation) =
        independently_validated_dead_scalar_shape(candidate.rule(), &node.operation)
            .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if source_operation != patch.source_operation
        || result != patch.result
        || scalar_type != patch.scalar_type
        || node.definitions
            != [ValueDefinition {
                value: result,
                scalar_type,
                site: ValueDefinitionSite::Node {
                    block: block.id,
                    node: patch.location.node,
                },
            }]
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
        || block.nodes.get(node_index + 1).is_none()
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if live.live_out.contains(&result)
        || function
            .node_uses()
            .any(|use_site| use_site.value == result)
    {
        return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
    }
    match (obligation, candidate.accepted_obligation_witness()) {
        (Some(obligation), Some(identity)) => {
            if !function.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference {
                        obligation: reference,
                        support,
                    } if *support == source_operation && *reference == obligation
                )
            }) || !input.accepted_obligation_facts.iter().any(|fact| {
                fact.identity == identity
                    && fact.machine == function.machine
                    && fact.owner == optimization_unit::PsiProvenance::Operation(source_operation)
                    && fact.obligation == obligation
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
        }
        (None, None) => {
            if function.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference { support, .. }
                        if *support == source_operation
                )
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
        }
        _ => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
    }
    let receiver = &block.nodes[node_index + 1];
    if receiver
        .provenance
        .iter()
        .any(|source| node.provenance.contains(source))
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_dead_scalar_node_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .function_mut(patch.location.machine)
        .expect("candidate function exists");
    let output_block = output_function
        .block_mut(patch.location.block)
        .expect("candidate block exists");
    let removed = output_block.nodes.remove(node_index);
    let receiver = output_block
        .nodes
        .get_mut(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&removed.provenance);
    receiver.fuel.extend_from_slice(&removed.fuel);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            rebuild_node_metadata(node, block.id, node_index)?;
            effect = set_node_effect_link(node, effect)?;
        }
    }
    let output_function =
        crate::unit_validation::revalidate_rewrite_output(&mut output, patch.location.machine)?;
    validate_outside_region(&function.blocks, output_function, &expected_blocks)?;
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: super::rule_catalog::validator_identity(candidate.rule())
            .expect("the entrance rejects unknown dead-scalar rules"),
        provenance: accepted_provenance,
    })
}
