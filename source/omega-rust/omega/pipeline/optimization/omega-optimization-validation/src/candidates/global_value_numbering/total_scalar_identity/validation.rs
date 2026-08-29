//! Candidate contract, shape, evidence, and accounting join.

use super::application::independently_apply_total_scalar_identity;
use super::classification::independently_classify_total_scalar_identity;
use super::evidence::independently_validate_neutral_literal;
use super::*;

const RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1";

pub fn validate_total_scalar_identity_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let rule = OptimizationRuleIdentity::from_canonical_bytes(RULE_DOMAIN);
    if candidate.rule() != rule
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::ScalarConstants)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::ExactOperationSemantics
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
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
    independently_validate_neutral_literal(input, function, shape, candidate)?;
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
    )
}
