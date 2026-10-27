//! Independent dead-node rewrite reconstruction and application.
use crate::candidates::rewrite_accounting::dead_scalar::reconstruct_dead_scalar_node_accounting;
use crate::{
    OptimizationUnitValidationError, ValidatedPsiRewrite, reconstruct_closed_scalar_node_boundary,
};
use optimization_unit::{
    OptimizationFact, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch,
};

use super::operation_partition::independently_validated_dead_scalar_shape;
use crate::candidates::{
    accept_region_rewrite, candidate_block, candidate_function, receiver_custody_distinct,
    retirable_scalar_node, retire_node_custody, sweep_output_nodes,
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
        || !retirable_scalar_node(block, node_index, node, result, scalar_type)
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
            }) || input
                .accepted_obligation_fact(
                    function.machine,
                    optimization_unit::PsiProvenance::Operation(source_operation),
                    Some(obligation),
                )
                .is_none_or(|fact| fact.identity != identity)
            {
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
    if !receiver_custody_distinct(block, node_index, node) {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_dead_scalar_node_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;

    accept_region_rewrite(
        input,
        candidate,
        patch.location.machine,
        &expected_blocks,
        super::rule_catalog::validator_bytes(candidate.rule())
            .expect("the entrance rejects unknown dead-scalar rules"),
        accepted_provenance,
        |output| {
            let output_function = output
                .function_mut(patch.location.machine)
                .expect("candidate function exists");
            let output_block = output_function
                .block_mut(patch.location.block)
                .expect("candidate block exists");
            retire_node_custody(output_block, node_index)?;
            sweep_output_nodes(output_function, |_| Ok(()))?;
            Ok(())
        },
    )
}
