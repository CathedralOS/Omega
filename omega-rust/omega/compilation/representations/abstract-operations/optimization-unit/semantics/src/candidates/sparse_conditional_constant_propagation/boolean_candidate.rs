//! Boolean constant-evaluation candidate acceptance.
use crate::candidates::provenance_realized_at;
use crate::candidates::sparse_conditional_constant_propagation::{
    boolean_evaluation, observation_at,
};
use crate::candidates::{candidate_block, candidate_function};
use crate::{
    OptimizationUnitValidationError, ValidatedPsiRewrite, reconstruct_closed_scalar_node_boundary,
    validate_psi_optimization_unit,
};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationValidatorIdentity,
};
use optimization_unit::{
    BooleanConstantRewrite, IntegerEvaluationWitness, ProvenanceDisposition, PsiOptimizationUnit,
    PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch,
};
use semantic_vocabulary::ScalarType;

pub(crate) fn validate_boolean_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
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
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = candidate_function(input, patch.location.machine)?;
    let block = candidate_block(function, patch.location.block)?;
    let node = block
        .nodes
        .get(usize::try_from(patch.location.node).expect("u32 fits usize"))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let input_observation = observation_at(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [provenance] = candidate.provenance() else {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    };
    let site = PsiRealizationSite::Node(patch.location);
    if provenance.input != site
        || provenance.disposition != ProvenanceDisposition::RealizedAt(site)
        || provenance.sources != node.provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if provenance.fuel != node.fuel {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }
    let accepted_provenance = vec![provenance_realized_at(
        site,
        site,
        &node.provenance,
        &node.fuel,
    )];
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
