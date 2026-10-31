//! Proof-certified scalar identity candidate acceptance.
use crate::candidates::rewrite_accounting::scalar_identity::reconstruct_proof_certified_scalar_identity_accounting;
use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
use crate::candidates::sparse_conditional_constant_propagation::scalar_value_definition;
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_core::{AnalysisKind, OptimizationSafetyClass};
use optimization_unit::{
    OptimizationFact, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch,
    ScalarConstantValue, ScalarSubstitution, literal_scalar_constant_fact_identity,
};
use semantic_vocabulary::ScalarType;

use super::identity_classification::independent_proof_certified_scalar_identity;
use super::literal_operand::materialized_integer_constant;
use super::rule_catalog::{is_operand_substitution_rule, operand_substitution_row};
use crate::candidates::{
    accept_region_rewrite, candidate_block, candidate_function, contract, retire_node_custody,
    sweep_output_nodes,
};
use optimization_unit::{receiver_custody_distinct, retirable_scalar_node};

/// Independently remove one live proof-certified integer identity.
/// Accepted proof and literal evidence are reconstructed from immutable input
/// custody; the declared operation is deleted rather than reclassified.
pub(crate) fn validate_proof_certified_scalar_identity_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    contract::admit_origin(input, candidate)?;
    if !is_operand_substitution_rule(candidate.rule()) {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    contract::admit_roster(
        candidate,
        &[
            AnalysisKind::ScalarConstants,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ],
        contract::USE_DEFINITION_AND_EFFECT_SUMMARIES,
        OptimizationSafetyClass::ProofCertified,
        Some(-1),
    )?;
    candidate_patch!(patch, EliminateProofCertifiedScalarIdentity, candidate);
    let validator = operand_substitution_row(candidate.rule(), patch.identity)
        .map(|row| row.validator_domain)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if candidate.node_decision_point() != Some(patch.location)
        || candidate.substitutions()
            != [ScalarSubstitution {
                from: patch.result,
                to: patch.replacement,
                scalar_type: ScalarType::Integer(patch.scalar_type),
            }]
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = candidate_function(input, patch.location.machine)?;
    let block = candidate_block(function, patch.location.block)?;
    let node_index = usize::try_from(patch.location.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = crate::candidates::candidate_node(block, patch.location.node)?;
    let scalar_type = ScalarType::Integer(patch.scalar_type);
    let shape = independent_proof_certified_scalar_identity(&node.operation, patch.identity)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if shape.source_operation != patch.source_operation
        || shape.result != patch.result
        || shape.replacement != patch.replacement
        || shape.result_type != patch.scalar_type
        || !retirable_scalar_node(block, node_index, node, patch.result, scalar_type)
        || !function
            .node_uses()
            .any(|use_site| use_site.value == patch.result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if scalar_value_definition(function, shape.replacement)
        .is_none_or(|definition| definition.scalar_type != ScalarType::Integer(shape.result_type))
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let identity_definition = scalar_value_definition(function, shape.identity_operand)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if identity_definition.scalar_type != ScalarType::Integer(shape.identity_type) {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let literal = materialized_integer_constant(function, identity_definition)?;
    if literal.result != shape.identity_operand
        || literal.scalar_type != ScalarType::Integer(shape.identity_type)
        || literal.value != shape.identity_constant
        || !function.facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::IntegerConstant { value, constant, support }
                if *value == shape.identity_operand
                    && *constant == shape.identity_constant
                    && *support == literal.operation)
        })
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let expected_constant_fact = literal_scalar_constant_fact_identity(
        input.identity,
        function.machine,
        identity_definition,
        ScalarConstantValue::Integer(shape.identity_constant),
        literal.operation,
    )
    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let Some((constant_fact, obligation_fact)) =
        candidate.proof_certified_scalar_identity_witness()
    else {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    };
    if constant_fact != expected_constant_fact {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    if !function.facts.iter().any(|fact| {
        matches!(fact, OptimizationFact::OperationObligationReference { obligation, support }
            if *obligation == shape.obligation && *support == shape.source_operation)
    }) || input
        .accepted_obligation_fact(
            function.machine,
            optimization_unit::PsiProvenance::Operation(shape.source_operation),
            Some(shape.obligation),
        )
        .is_none_or(|fact| fact.identity != obligation_fact)
    {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
    if !receiver_custody_distinct(block, node_index, node) {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_proof_certified_scalar_identity_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;

    accept_region_rewrite(
        input,
        candidate,
        patch.location.machine,
        &expected_blocks,
        validator,
        accepted_provenance,
        |output| {
            let output_function = output
                .function_mut(patch.location.machine)
                .expect("candidate function exists");
            let output_block = output_function
                .block_mut(patch.location.block)
                .expect("candidate block exists");
            retire_node_custody(output_block, node_index)?;
            sweep_output_nodes(output_function, |node| {
                rewrite_scalar_value_uses(&mut node.operation, patch.result, patch.replacement);
                Ok(())
            })?;
            // The rewrite deletes one node and moves no accepted obligation
            // fact, so any catalog drift is the candidate's claim alone.
            if output.accepted_obligation_facts != input.accepted_obligation_facts {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            Ok(())
        },
    )
}
