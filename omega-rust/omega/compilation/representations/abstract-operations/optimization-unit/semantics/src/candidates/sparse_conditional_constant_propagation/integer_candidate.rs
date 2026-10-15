//! Integer constant-evaluation candidate acceptance.
use crate::candidates::provenance_realized_at;
use crate::candidates::sparse_conditional_constant_propagation::observation_at;
use crate::{
    OptimizationUnitValidationError, ValidatedPsiRewrite, reconstruct_closed_scalar_node_boundary,
    validate_psi_optimization_unit,
};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationSafetyClass,
    OptimizationValidatorIdentity,
};
use optimization_unit::{
    IntegerConstantRewrite, IntegerEvaluationWitness, OptimizationFact, ProvenanceDisposition,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch,
};
use semantic_vocabulary::ScalarType;

use super::integer_evaluation::evaluate_integer_operation;
use crate::candidates::{candidate_block, candidate_function};

pub(crate) fn validate_integer_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if candidate.required_analyses() != AnalysisSet::new([AnalysisKind::ScalarConstants])
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new([AnalysisKind::UseDefinition])
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
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

    let (source_operation, result, scalar_type, evaluated, safety_class) =
        evaluate_integer_operation(function, node, candidate)?;
    if candidate.safety_class() != safety_class {
        return Err(OptimizationUnitValidationError::CandidateSafetyClassMismatch);
    }
    match (
        safety_class,
        candidate
            .scalar_evaluation_witness()
            .and_then(IntegerEvaluationWitness::obligation_fact),
    ) {
        (OptimizationSafetyClass::ProofCertified, Some(identity)) => {
            let fact = input
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.identity == identity
                        && fact.machine == function.machine
                        && fact.owner
                            == optimization_unit::PsiProvenance::Operation(source_operation)
                })
                .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if !function.facts.iter().any(|reference| {
                matches!(
                    reference,
                    OptimizationFact::OperationObligationReference { obligation, support }
                        if *support == source_operation && *obligation == fact.obligation
                )
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
        }
        (OptimizationSafetyClass::ProofCertified, None) | (_, Some(_)) => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
        (_, None) => {}
    }
    if patch
        != (IntegerConstantRewrite {
            location: patch.location,
            source_operation,
            result,
            scalar_type,
            constant: evaluated,
        })
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }
    let output = super::constant_installation::install_evaluated_constant(
        input,
        patch.location,
        abstract_operations::AbstractOperation::IntegerConstant {
            psi_operation: patch.source_operation,
            result: patch.result,
            scalar_type: ScalarType::Integer(patch.scalar_type),
            value: patch.constant,
        },
        patch.result,
        ScalarType::Integer(patch.scalar_type),
        &input_observation,
        &input_live,
    )?;
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.exact-integer-evaluation.v2",
        ),
        provenance: accepted_provenance,
    })
}
