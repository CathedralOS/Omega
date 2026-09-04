//! Boolean constant-evaluation candidate acceptance.

use super::*;

pub fn validate_boolean_evaluation_candidate(
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
    let accepted_provenance = vec![omega_optimization_unit::ProvenanceRewrite {
        input: site,
        disposition: ProvenanceDisposition::RealizedAt(site),
        sources: node.provenance.clone(),
        fuel: node.fuel.clone(),
    }];
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
    let mut output = input.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate source function exists");
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate source block exists");
    let node = &mut block.nodes[usize::try_from(patch.location.node).expect("u32 fits usize")];
    node.operation = omega_abstract_operations::AbstractOperation::BooleanConstant {
        psi_operation: patch.source_operation,
        result: patch.result,
        value: patch.constant,
    };
    node.definitions = vec![ValueDefinition {
        value: patch.result,
        scalar_type: ScalarType::Boolean,
        site: ValueDefinitionSite::Node {
            block: patch.location.block,
            node: patch.location.node,
        },
    }];
    node.uses.clear();
    node.successors.clear();
    node.ownership.clear();
    function.facts = reconstruct_fact_index(function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_observation = observation_at(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !same_closed_scalar_observation(&input_observation, &output_observation) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let output_live = reconstruct_closed_scalar_node_boundary(&output, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if input_live.live_out != output_live.live_out
        || output_live
            .live_in
            .iter()
            .any(|value| !input_live.live_in.contains(value))
    {
        return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.boolean-evaluation.v4",
        ),
        provenance: accepted_provenance,
    })
}
