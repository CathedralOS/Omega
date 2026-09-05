//! Candidate contract, shape, evidence, and accounting join.

use super::application::independently_apply_total_scalar_identity;
use super::classification::independently_classify_total_scalar_identity;
use super::evidence::independently_validate_law_literal;
use super::*;

const NEUTRAL_ARITHMETIC_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1";
const SHIFT_ZERO_COUNT_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1";
const MULTIPLY_ZERO_ANNIHILATION_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1";
const SATURATING_NEUTRAL_ARITHMETIC_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1";
const SATURATING_MULTIPLY_ZERO_ANNIHILATION_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-saturating-integer-multiply-zero-annihilation.v1";
const BITWISE_NEUTRAL_LITERAL_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-integer-bitwise-neutral-literal-elimination.v1";
const BITWISE_ABSORBING_LITERAL_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-integer-bitwise-absorbing-literal-elimination.v1";

pub fn validate_total_scalar_identity_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if candidate.required_analyses()
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
        || candidate.safety_class() != OptimizationSafetyClass::ExactOperationSemantics
        || candidate.predicted_cost_delta() != -1
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let validator = exact_rule_validator(candidate.rule(), patch.identity)
        .ok_or(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)?;
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
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node_index = usize::try_from(patch.location.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let shape = independently_classify_total_scalar_identity(&node.operation, patch.identity)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if shape.source_operation != patch.source_operation
        || shape.result != patch.result
        || shape.replacement != patch.replacement
        || shape.scalar_type != patch.scalar_type
        || node.definitions
            != [ValueDefinition {
                value: patch.result,
                scalar_type: ScalarType::Integer(patch.scalar_type),
                site: ValueDefinitionSite::Node {
                    block: block.id,
                    node: patch.location.node,
                },
            }]
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
        || block.nodes.get(node_index + 1).is_none()
        || !function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.uses)
            .any(|use_site| use_site.value == patch.result)
        || scalar_value_definition(function, shape.replacement).is_none_or(|definition| {
            definition.scalar_type != ScalarType::Integer(shape.scalar_type)
        })
        || function.facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == shape.source_operation)
        })
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    independently_validate_law_literal(input, function, shape, candidate)?;
    let receiver = &block.nodes[node_index + 1];
    if receiver
        .provenance
        .iter()
        .any(|source| node.provenance.contains(source))
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let (affected_blocks, provenance) =
        reconstruct_total_scalar_identity_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != affected_blocks || candidate.provenance() != provenance {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    independently_apply_total_scalar_identity(
        input,
        candidate,
        patch,
        node_index,
        &affected_blocks,
        provenance,
        validator,
    )
}

fn exact_rule_validator(
    rule: OptimizationRuleIdentity,
    identity: TotalScalarIdentityKind,
) -> Option<OptimizationValidatorIdentity> {
    let (rule_domain, validator_domain) = match identity {
        TotalScalarIdentityKind::WrappingIntegerAddZeroLeft
        | TotalScalarIdentityKind::WrappingIntegerAddZeroRight
        | TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight
        | TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft
        | TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight => (
            NEUTRAL_ARITHMETIC_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount
        | TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount => (
            SHIFT_ZERO_COUNT_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft
        | TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight => (
            MULTIPLY_ZERO_ANNIHILATION_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft
        | TotalScalarIdentityKind::SaturatingIntegerAddZeroRight
        | TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight => (
            SATURATING_NEUTRAL_ARITHMETIC_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight => (
            SATURATING_MULTIPLY_ZERO_ANNIHILATION_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-saturating-integer-multiply-zero-annihilation.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft
        | TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight
        | TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseOrZeroRight
        | TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseXorZeroRight => (
            BITWISE_NEUTRAL_LITERAL_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-integer-bitwise-neutral-literal-elimination.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseAndZeroRight
        | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft
        | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight => (
            BITWISE_ABSORBING_LITERAL_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-integer-bitwise-absorbing-literal-elimination.v1"
                .as_slice(),
        ),
    };
    (rule == OptimizationRuleIdentity::from_canonical_bytes(rule_domain))
        .then(|| OptimizationValidatorIdentity::from_canonical_bytes(validator_domain))
}
