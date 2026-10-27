//! Proof-certified scalar identity candidate acceptance.
use crate::candidates::rewrite_accounting::scalar_identity::reconstruct_proof_certified_scalar_identity_accounting;
use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
use crate::candidates::sparse_conditional_constant_propagation::scalar_value_definition;
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleIdentity,
    OptimizationSafetyClass,
};
use optimization_unit::{
    OptimizationFact, ProofCertifiedScalarIdentityKind, PsiOptimizationUnit, PsiRewriteCandidate,
    PsiRewritePatch, ScalarConstantValue, ScalarSubstitution, ValueDefinitionSite,
    literal_scalar_constant_fact_identity,
};
use semantic_vocabulary::ScalarType;

use super::identity_classification::independent_proof_certified_scalar_identity;
use crate::candidates::{
    accept_region_rewrite, candidate_block, candidate_function, receiver_custody_distinct,
    retirable_scalar_node, retire_node_custody, sweep_output_nodes,
};

/// Independently remove one live proof-certified integer identity.
/// Accepted proof and literal evidence are reconstructed from immutable input
/// custody; the declared operation is deleted rather than reclassified.
pub(crate) fn validate_proof_certified_scalar_identity_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let exact_identity_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-integer-identity-elimination.v1",
    );
    let divide_by_one_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-integer-divide-by-one-elimination.v1",
    );
    let multiply_by_zero_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1",
    );
    let zero_dividend_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-integer-zero-dividend-elimination.v1",
    );
    let zero_value_shift_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-exact-integer-zero-value-shift-elimination.v1",
    );
    let negative_one_shift_right_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1",
    );
    if ![
        exact_identity_rule,
        divide_by_one_rule,
        multiply_by_zero_rule,
        zero_dividend_rule,
        zero_value_shift_rule,
        negative_one_shift_right_rule,
    ]
    .contains(&candidate.rule())
        || candidate.required_analyses()
            != AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ])
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ])
        || candidate.safety_class() != OptimizationSafetyClass::ProofCertified
        || candidate.predicted_cost_delta() != -1
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let validator = match candidate.rule() {
        rule if rule == exact_identity_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight
                    | ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight
                    | ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight
                    | ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount
                    | ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount
            ) =>
        {
            b"omega.validator.live-proof-certified-integer-identity-elimination.v1".as_slice()
        }
        rule if rule == divide_by_one_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight
                    | ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight
                    | ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight
            ) =>
        {
            b"omega.validator.live-proof-certified-integer-divide-by-one-elimination.v1".as_slice()
        }
        rule if rule == multiply_by_zero_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight
            ) =>
        {
            b"omega.validator.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1"
                .as_slice()
        }
        rule if rule == zero_dividend_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft
                    | ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft
                    | ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft
                    | ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft
                    | ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft
            ) =>
        {
            b"omega.validator.live-proof-certified-integer-zero-dividend-elimination.v1".as_slice()
        }
        rule if rule == zero_value_shift_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue
                    | ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue
            ) =>
        {
            b"omega.validator.live-proof-certified-exact-integer-zero-value-shift-elimination.v1"
                .as_slice()
        }
        rule if rule == negative_one_shift_right_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue
            ) =>
        {
            b"omega.validator.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1"
                .as_slice()
        }
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
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
    let ValueDefinitionSite::Node {
        block: literal_block,
        node: literal_node,
    } = identity_definition.site
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let literal = function
        .block(literal_block)
        .and_then(|block| {
            usize::try_from(literal_node)
                .ok()
                .and_then(|node| block.nodes.get(node))
        })
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let O::IntegerConstant {
        psi_operation: literal_support,
        result: literal_result,
        scalar_type: literal_type,
        value: literal_value,
    } = literal.operation
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    if literal_result != shape.identity_operand
        || literal_type != ScalarType::Integer(shape.identity_type)
        || literal_value != shape.identity_constant
        || !function.facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::IntegerConstant { value, constant, support }
                if *value == shape.identity_operand
                    && *constant == shape.identity_constant
                    && *support == literal_support)
        })
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let expected_constant_fact = literal_scalar_constant_fact_identity(
        input.identity,
        function.machine,
        identity_definition,
        ScalarConstantValue::Integer(shape.identity_constant),
        literal_support,
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
