//! Boolean constant-evaluation candidate acceptance.
use crate::candidates::realized_at_own_site;
use crate::candidates::sparse_conditional_constant_propagation::{
    boolean_evaluation, observation_at,
};
use crate::candidates::{
    candidate_block, candidate_function, candidate_node, require_realized_rows,
};
use crate::{
    OptimizationUnitValidationError, ValidatedPsiRewrite, reconstruct_closed_scalar_node_boundary,
};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationValidatorIdentity,
};
use optimization_unit::{
    BooleanConstantRewrite, IntegerEvaluationWitness, PsiOptimizationUnit, PsiRealizationSite,
    PsiRewriteCandidate, PsiRewritePatch,
};
use semantic_vocabulary::ScalarType;

pub(crate) fn validate_boolean_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    crate::candidates::contract::admit_origin(input, candidate)?;
    let expected_analyses = match candidate.scalar_evaluation_witness() {
        Some(IntegerEvaluationWitness::RangeAgainstRange { .. }) => {
            AnalysisSet::new([AnalysisKind::ValueRanges])
        }
        Some(IntegerEvaluationWitness::RangeAgainstConstant { .. }) => {
            AnalysisSet::new([AnalysisKind::ScalarConstants, AnalysisKind::ValueRanges])
        }
        Some(_) => AnalysisSet::new([AnalysisKind::ScalarConstants]),
        None => return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    };
    if candidate.required_analyses() != expected_analyses
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new([AnalysisKind::UseDefinition])
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    candidate_patch!(patch, ReplaceBooleanOperationWithConstant, candidate);
    if candidate.node_decision_point() != Some(patch.location) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = candidate_function(input, patch.location.machine)?;
    let block = candidate_block(function, patch.location.block)?;
    let node = candidate_node(block, patch.location.node)?;
    let input_observation = observation_at(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let site = PsiRealizationSite::Node(patch.location);
    let accepted_provenance = vec![realized_at_own_site(site, &node.provenance, &node.fuel)];
    require_realized_rows(candidate, &accepted_provenance)?;
    let (source_operation, result, evaluated, safety_class) =
        boolean_evaluation::evaluate(input, function, node, candidate, patch.location)?;
    if candidate.safety_class() != safety_class {
        return Err(OptimizationUnitValidationError::CandidateSafetyClassMismatch);
    }
    if patch
        != (BooleanConstantRewrite {
            location: patch.location,
            source_operation,
            result,
            constant: evaluated,
        })
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }
    let output = crate::candidates::constant_installation::install_evaluated_constant(
        input,
        patch.location,
        abstract_operations::AbstractOperation::BooleanConstant {
            psi_operation: patch.source_operation,
            result: patch.result,
            value: patch.constant,
        },
        patch.result,
        ScalarType::Boolean,
        &input_observation,
        &input_live,
    )?;
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.boolean-evaluation.v4",
        ),
        provenance: accepted_provenance,
    })
}
