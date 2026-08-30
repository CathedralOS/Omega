//! Boolean constant-evaluation candidate acceptance.

use super::integer_evaluation::*;
use super::range_comparisons::*;
use super::snapshot_reconstruction::validator_integer_value_type;
use super::*;

pub fn validate_boolean_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let has_required_evaluation_analysis = match candidate.scalar_evaluation_witness() {
        Some(IntegerEvaluationWitness::RangeAgainstRange { .. }) => candidate
            .required_analyses()
            .contains(AnalysisKind::ValueRanges),
        Some(_) => candidate
            .required_analyses()
            .contains(AnalysisKind::ScalarConstants),
        None => false,
    };
    if !has_required_evaluation_analysis
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
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
        evaluate_boolean_operation(input, function, node, candidate, patch.location)?;
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

pub(crate) fn evaluate_boolean_operation(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    node: &omega_optimization_unit::OptimizationNode,
    candidate: &PsiRewriteCandidate,
    location: NodeLocation,
) -> Result<
    (
        psi_core::OperationId,
        ValueId,
        bool,
        OptimizationSafetyClass,
    ),
    OptimizationUnitValidationError,
> {
    use omega_abstract_operations::AbstractOperation as O;
    match node.operation {
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => {
            let Some(operand_fact) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::unary_operand)
            else {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            };
            let operand = literal_boolean_fact(function, candidate.input(), operand, operand_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            Ok((
                psi_operation,
                result,
                !operand,
                OptimizationSafetyClass::ExactOperationSemantics,
            ))
        }
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let Some((left_fact, right_fact)) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::binary_operands)
            else {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            };
            let left = literal_boolean_fact(function, candidate.input(), left, left_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let right = literal_boolean_fact(function, candidate.input(), right, right_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            Ok((
                psi_operation,
                result,
                left == right,
                OptimizationSafetyClass::ExactOperationSemantics,
            ))
        }
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        }
        | O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        }
        | O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            if let Some((left_range_fact, right_range_fact)) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::range_against_range)
            {
                if !candidate
                    .required_analyses()
                    .contains(AnalysisKind::ValueRanges)
                {
                    return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
                }
                let kind = independently_validated_integer_range_pair_comparison_kind(
                    candidate.rule(),
                    &node.operation,
                )
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                let left_range =
                    current_value_ranges::independently_reconstruct_value_range_fact_at(
                        input,
                        left_range_fact,
                        function.machine,
                        left,
                        location.block,
                        location.node,
                    )
                    .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
                let right_range =
                    current_value_ranges::independently_reconstruct_value_range_fact_at(
                        input,
                        right_range_fact,
                        function.machine,
                        right,
                        location.block,
                        location.node,
                    )
                    .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
                if left_range.scalar_type != right_range.scalar_type
                    || validator_integer_value_type(function, left) != Some(left_range.scalar_type)
                    || validator_integer_value_type(function, right)
                        != Some(right_range.scalar_type)
                {
                    return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
                }
                let constant = independently_evaluate_integer_range_pair_comparison(
                    kind,
                    left_range.scalar_type,
                    left == right,
                    left_range.minimum,
                    left_range.maximum,
                    right_range.minimum,
                    right_range.maximum,
                )
                .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
                return Ok((
                    psi_operation,
                    result,
                    constant,
                    OptimizationSafetyClass::ProofCertified,
                ));
            }
            if let Some((range_fact, constant_fact)) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::range_against_constant)
            {
                if !candidate
                    .required_analyses()
                    .contains(AnalysisKind::ValueRanges)
                {
                    return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
                }
                let kind = independently_validated_integer_range_comparison_kind(
                    candidate.rule(),
                    &node.operation,
                )
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                let (range_operand, constant_operand) = match kind {
                    ValidatedIntegerRangeComparisonKind::RangeEqualConstant
                    | ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
                    | ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant => {
                        (left, right)
                    }
                    ValidatedIntegerRangeComparisonKind::ConstantEqualRange
                    | ValidatedIntegerRangeComparisonKind::ConstantLessThanRange
                    | ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange => {
                        (right, left)
                    }
                };
                let constant_value = direct_literal_integer_fact(
                    function,
                    candidate.input(),
                    constant_operand,
                    constant_fact,
                )
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                let range = current_value_ranges::independently_reconstruct_value_range_fact_at(
                    input,
                    range_fact,
                    function.machine,
                    range_operand,
                    location.block,
                    location.node,
                )
                .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
                if validator_integer_value_type(function, constant_operand)
                    != Some(range.scalar_type)
                {
                    return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
                }
                let constant = independently_evaluate_integer_range_comparison(
                    kind,
                    range.scalar_type,
                    range.minimum,
                    range.maximum,
                    constant_value,
                )
                .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
                return Ok((
                    psi_operation,
                    result,
                    constant,
                    OptimizationSafetyClass::ProofCertified,
                ));
            }
            let Some((left_fact, right_fact)) = candidate
                .scalar_evaluation_witness()
                .and_then(IntegerEvaluationWitness::binary_operands)
            else {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            };
            let left_value = literal_integer_fact(function, candidate.input(), left, left_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let right_value = literal_integer_fact(function, candidate.input(), right, right_fact)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let left_type = validator_integer_value_type(function, left)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            if validator_integer_value_type(function, right) != Some(left_type) {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            }
            let ordering = left_type
                .compare(left_value, right_value)
                .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
            let constant = match node.operation {
                O::IntegerEqual { .. } => ordering.is_eq(),
                O::IntegerLessThan { .. } => ordering.is_lt(),
                O::IntegerLessOrEqual { .. } => !ordering.is_gt(),
                _ => unreachable!(),
            };
            Ok((
                psi_operation,
                result,
                constant,
                OptimizationSafetyClass::ExactOperationSemantics,
            ))
        }
        _ => Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    }
}
