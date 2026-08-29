//! SCCP scalar, boolean, range, and snapshot validation.

use super::*;

pub fn validate_integer_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ScalarConstants)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
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
                        && fact.operation == source_operation
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
    node.operation = omega_abstract_operations::AbstractOperation::IntegerConstant {
        psi_operation: patch.source_operation,
        result: patch.result,
        scalar_type: ScalarType::Integer(patch.scalar_type),
        value: patch.constant,
    };
    node.definitions = vec![ValueDefinition {
        value: patch.result,
        scalar_type: ScalarType::Integer(patch.scalar_type),
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
            b"omega.validator.exact-integer-evaluation.v2",
        ),
        provenance: accepted_provenance,
    })
}

pub fn validate_scalar_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1",
        )
    {
        return validate_proof_certified_exact_integer_self_subtract_candidate(input, candidate);
    }
    if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-integer-self-remainder-elimination.v1",
        )
    {
        return validate_proof_certified_integer_self_remainder_candidate(input, candidate);
    }
    if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-integer-self-divide-elimination.v1",
        )
    {
        return validate_proof_certified_integer_self_divide_candidate(input, candidate);
    }
    if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1",
        )
    {
        return validate_proof_certified_integer_remainder_by_one_candidate(input, candidate);
    }
    if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
        )
    {
        return validate_proof_certified_signed_integer_remainder_by_negative_one_candidate(
            input, candidate,
        );
    }
    match candidate.patch() {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(_) => {
            validate_integer_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
            validate_boolean_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::FoldConstantConditional(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
        PsiRewritePatch::MergeAdjacentBlock(_)
        | PsiRewritePatch::MergeNonAdjacentBlock(_)
        | PsiRewritePatch::FuseSharedTerminalJump(_)
        | PsiRewritePatch::RemoveDeadScalarNode(_)
        | PsiRewritePatch::EliminateLocalScalarCommonSubexpression(_)
        | PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(_)
        | PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(_)
        | PsiRewritePatch::EliminateProofCertifiedScalarIdentity(_)
        | PsiRewritePatch::PruneUnreachablePrivateMachines(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
    }
}

/// Dispatch one closed Psi rewrite candidate to a patch-specific independent
/// validator. Rules cannot construct accepted outputs themselves.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidatedIntegerRangeComparisonKind {
    RangeEqualConstant,
    ConstantEqualRange,
    RangeLessThanConstant,
    ConstantLessThanRange,
    RangeLessOrEqualConstant,
    ConstantLessOrEqualRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidatedIntegerRangePairComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

pub(crate) fn independently_validated_integer_range_pair_comparison_kind(
    rule: OptimizationRuleIdentity,
    operation: &O,
) -> Option<ValidatedIntegerRangePairComparisonKind> {
    let kind = if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-equal-range-range.v1",
        ) {
        ValidatedIntegerRangePairComparisonKind::Equal
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-than-range-range.v1",
        )
    {
        ValidatedIntegerRangePairComparisonKind::LessThan
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-or-equal-range-range.v1",
        )
    {
        ValidatedIntegerRangePairComparisonKind::LessOrEqual
    } else {
        return None;
    };
    match (kind, operation) {
        (ValidatedIntegerRangePairComparisonKind::Equal, O::IntegerEqual { .. })
        | (ValidatedIntegerRangePairComparisonKind::LessThan, O::IntegerLessThan { .. })
        | (ValidatedIntegerRangePairComparisonKind::LessOrEqual, O::IntegerLessOrEqual { .. }) => {
            Some(kind)
        }
        _ => None,
    }
}

pub(crate) fn independently_evaluate_integer_range_pair_comparison(
    kind: ValidatedIntegerRangePairComparisonKind,
    scalar_type: psi_core::IntegerType,
    same_value: bool,
    left_minimum: psi_core::IntegerValue,
    left_maximum: psi_core::IntegerValue,
    right_minimum: psi_core::IntegerValue,
    right_maximum: psi_core::IntegerValue,
) -> Option<bool> {
    if same_value {
        return Some(!matches!(
            kind,
            ValidatedIntegerRangePairComparisonKind::LessThan
        ));
    }
    let left_maximum_to_right_minimum = scalar_type.compare(left_maximum, right_minimum)?;
    let left_minimum_to_right_maximum = scalar_type.compare(left_minimum, right_maximum)?;
    match kind {
        ValidatedIntegerRangePairComparisonKind::Equal => {
            let both_equal_singletons = scalar_type.compare(left_minimum, left_maximum)?.is_eq()
                && scalar_type.compare(right_minimum, right_maximum)?.is_eq()
                && scalar_type.compare(left_minimum, right_minimum)?.is_eq();
            both_equal_singletons.then_some(true).or_else(|| {
                (left_maximum_to_right_minimum.is_lt() || left_minimum_to_right_maximum.is_gt())
                    .then_some(false)
            })
        }
        ValidatedIntegerRangePairComparisonKind::LessThan => left_maximum_to_right_minimum
            .is_lt()
            .then_some(true)
            .or_else(|| (!left_minimum_to_right_maximum.is_lt()).then_some(false)),
        ValidatedIntegerRangePairComparisonKind::LessOrEqual => (!left_maximum_to_right_minimum
            .is_gt())
        .then_some(true)
        .or_else(|| left_minimum_to_right_maximum.is_gt().then_some(false)),
    }
}

pub(crate) fn independently_validated_integer_range_comparison_kind(
    rule: OptimizationRuleIdentity,
    operation: &O,
) -> Option<ValidatedIntegerRangeComparisonKind> {
    let kind = if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-equal-range-constant.v1",
        ) {
        ValidatedIntegerRangeComparisonKind::RangeEqualConstant
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-equal-constant-range.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::ConstantEqualRange
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-than-range-constant.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-than-constant-range.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::ConstantLessThanRange
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-or-equal-range-constant.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.integer-less-or-equal-constant-range.v1",
        )
    {
        ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange
    } else {
        return None;
    };
    match (kind, operation) {
        (
            ValidatedIntegerRangeComparisonKind::RangeEqualConstant
            | ValidatedIntegerRangeComparisonKind::ConstantEqualRange,
            O::IntegerEqual { .. },
        )
        | (
            ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
            | ValidatedIntegerRangeComparisonKind::ConstantLessThanRange,
            O::IntegerLessThan { .. },
        )
        | (
            ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant
            | ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange,
            O::IntegerLessOrEqual { .. },
        ) => Some(kind),
        _ => None,
    }
}

pub(crate) fn independently_evaluate_integer_range_comparison(
    kind: ValidatedIntegerRangeComparisonKind,
    scalar_type: psi_core::IntegerType,
    minimum: psi_core::IntegerValue,
    maximum: psi_core::IntegerValue,
    constant: psi_core::IntegerValue,
) -> Option<bool> {
    let minimum_to_constant = scalar_type.compare(minimum, constant)?;
    let maximum_to_constant = scalar_type.compare(maximum, constant)?;
    match kind {
        ValidatedIntegerRangeComparisonKind::RangeEqualConstant
        | ValidatedIntegerRangeComparisonKind::ConstantEqualRange => {
            if minimum_to_constant.is_eq() && maximum_to_constant.is_eq() {
                Some(true)
            } else if minimum_to_constant.is_gt() || maximum_to_constant.is_lt() {
                Some(false)
            } else {
                None
            }
        }
        ValidatedIntegerRangeComparisonKind::RangeLessThanConstant => maximum_to_constant
            .is_lt()
            .then_some(true)
            .or_else(|| (!minimum_to_constant.is_lt()).then_some(false)),
        ValidatedIntegerRangeComparisonKind::ConstantLessThanRange => minimum_to_constant
            .is_gt()
            .then_some(true)
            .or_else(|| (!maximum_to_constant.is_gt()).then_some(false)),
        ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant => (!maximum_to_constant
            .is_gt())
        .then_some(true)
        .or_else(|| minimum_to_constant.is_gt().then_some(false)),
        ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange => (!minimum_to_constant
            .is_lt())
        .then_some(true)
        .or_else(|| maximum_to_constant.is_lt().then_some(false)),
    }
}

pub(crate) fn observation_at(
    unit: &PsiOptimizationUnit,
    location: omega_optimization_unit::NodeLocation,
) -> Option<PsiNodeObservation> {
    reconstruct_psi_observation_model(unit)
        .nodes
        .into_iter()
        .find(|row| {
            row.machine == location.machine
                && row.block == location.block
                && row.node == location.node
        })
}

pub(crate) fn same_closed_scalar_observation(
    input: &PsiNodeObservation,
    output: &PsiNodeObservation,
) -> bool {
    input.machine == output.machine
        && input.block == output.block
        && input.node == output.node
        && input.definitions == output.definitions
        && input.effect == output.effect
        && input.ownership == output.ownership
        && input.provenance == output.provenance
        && input.fuel == output.fuel
        && input.crash == output.crash
        && input.suspension == output.suspension
        && input.events == output.events
}

pub(crate) fn evaluate_integer_operation(
    function: &PsiOptimizationFunction,
    node: &omega_optimization_unit::OptimizationNode,
    candidate: &PsiRewriteCandidate,
) -> Result<
    (
        psi_core::OperationId,
        ValueId,
        psi_core::IntegerType,
        psi_core::IntegerValue,
        OptimizationSafetyClass,
    ),
    OptimizationUnitValidationError,
> {
    use omega_abstract_operations::AbstractOperation as O;
    if let O::IntegerExactCast {
        psi_operation,
        result,
        source_type,
        target_type,
        operand,
        ..
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = source_type
            .exact_cast_value_to(target_type, operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            target_type,
            evaluated,
            OptimizationSafetyClass::ProofCertified,
        ));
    }
    if let O::IntegerWiden {
        psi_operation,
        result,
        source_type,
        target_type,
        operand,
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = source_type
            .widen_value_to(target_type, operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            target_type,
            evaluated,
            OptimizationSafetyClass::ExactOperationSemantics,
        ));
    }
    if let O::IntegerBitwiseNot {
        psi_operation,
        result,
        scalar_type,
        operand,
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = scalar_type
            .bitwise_not(operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            scalar_type,
            evaluated,
            OptimizationSafetyClass::ExactOperationSemantics,
        ));
    }
    enum IntegerOperation {
        ExactAdd,
        ExactSubtract,
        ExactMultiply,
        WrappingAdd,
        WrappingSubtract,
        WrappingMultiply,
        SaturatingAdd,
        SaturatingSubtract,
        SaturatingMultiply,
        ExactDivide,
        ExactRemainder,
        WrappingDivide,
        WrappingRemainder,
        SaturatingDivide,
        SaturatingRemainder,
        ExactShiftLeft(psi_core::IntegerType),
        ExactShiftRight(psi_core::IntegerType),
        WrappingShiftLeft(psi_core::IntegerType),
        WrappingShiftRight(psi_core::IntegerType),
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
    }
    let (kind, source, result, scalar_type, left, right) = match &node.operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::WrappingDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::WrappingRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::SaturatingDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::SaturatingRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            IntegerOperation::ExactShiftLeft(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            IntegerOperation::ExactShiftRight(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IntegerOperation::WrappingShiftLeft(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IntegerOperation::WrappingShiftRight(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseAnd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseOr,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseXor,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
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
    let (evaluated, safety_class) = match kind {
        IntegerOperation::ExactAdd => (
            scalar_type.exact_add(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactSubtract => (
            scalar_type.exact_sub(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactMultiply => (
            scalar_type.exact_mul(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingAdd => (
            scalar_type.wrapping_add(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingSubtract => (
            scalar_type.wrapping_sub(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingMultiply => (
            scalar_type.wrapping_mul(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingAdd => (
            scalar_type.saturating_add(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingSubtract => (
            scalar_type.saturating_sub(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingMultiply => (
            scalar_type.saturating_mul(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::ExactDivide => (
            scalar_type.exact_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactRemainder => (
            scalar_type.exact_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingDivide => (
            scalar_type.wrapping_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingRemainder => (
            scalar_type.wrapping_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::SaturatingDivide => (
            scalar_type.saturating_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::SaturatingRemainder => (
            scalar_type.saturating_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactShiftLeft(count_type) => (
            scalar_type.exact_shift_left(left_value, count_type, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactShiftRight(count_type) => (
            scalar_type.exact_shift_right(left_value, count_type, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingShiftLeft(count_type) => (
            scalar_type.wrapping_shift_left(left_value, count_type, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingShiftRight(count_type) => (
            scalar_type.wrapping_shift_right(left_value, count_type, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseAnd => (
            scalar_type.bitwise_and(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseOr => (
            scalar_type.bitwise_or(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseXor => (
            scalar_type.bitwise_xor(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
    };
    let evaluated =
        evaluated.ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
    Ok((source, result, scalar_type, evaluated, safety_class))
}

pub(crate) fn unary_integer_operand(
    function: &PsiOptimizationFunction,
    candidate: &PsiRewriteCandidate,
    operand: ValueId,
) -> Result<psi_core::IntegerValue, OptimizationUnitValidationError> {
    let Some(operand_fact) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::unary_operand)
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    literal_integer_fact(function, candidate.input(), operand, operand_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)
}

pub(crate) fn literal_integer_fact(
    function: &PsiOptimizationFunction,
    input: omega_optimization_core::OptimizationUnitIdentity,
    value: ValueId,
    identity: omega_optimization_core::ScalarConstantFactIdentity,
) -> Option<psi_core::IntegerValue> {
    validator_scalar_constant_facts(input, function)
        .into_iter()
        .find_map(|(fact_value, constant, fact_identity)| {
            (fact_value == value && fact_identity == identity)
                .then_some(constant)
                .and_then(|constant| match constant {
                    ScalarConstantValue::Integer(value) => Some(value),
                    ScalarConstantValue::Boolean(_) => None,
                })
        })
}

pub(crate) fn direct_literal_integer_fact(
    function: &PsiOptimizationFunction,
    input: omega_optimization_core::OptimizationUnitIdentity,
    value: ValueId,
    identity: omega_optimization_core::ScalarConstantFactIdentity,
) -> Option<psi_core::IntegerValue> {
    let definition = scalar_value_definition(function, value)?;
    let ValueDefinitionSite::Node { block, node } = definition.site else {
        return None;
    };
    let operation = &function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?
        .nodes
        .get(usize::try_from(node).ok()?)?
        .operation;
    let O::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value: constant,
    } = operation
    else {
        return None;
    };
    if *result != value || *scalar_type != definition.scalar_type {
        return None;
    }
    let expected = literal_scalar_constant_fact_identity(
        input,
        function.machine,
        definition,
        ScalarConstantValue::Integer(*constant),
        *psi_operation,
    )?;
    (identity == expected).then_some(*constant)
}

pub(crate) fn literal_boolean_fact(
    function: &PsiOptimizationFunction,
    input: omega_optimization_core::OptimizationUnitIdentity,
    value: ValueId,
    identity: omega_optimization_core::ScalarConstantFactIdentity,
) -> Option<bool> {
    validator_scalar_constant_facts(input, function)
        .into_iter()
        .find_map(|(fact_value, constant, fact_identity)| {
            (fact_value == value && fact_identity == identity)
                .then_some(constant)
                .and_then(|constant| match constant {
                    ScalarConstantValue::Boolean(value) => Some(value),
                    ScalarConstantValue::Integer(_) => None,
                })
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidatorSccpValue {
    Unknown,
    Constant(ScalarConstantValue),
    Overdefined,
}

pub(crate) fn validator_scalar_constant_facts(
    input: omega_optimization_core::OptimizationUnitIdentity,
    function: &PsiOptimizationFunction,
) -> Vec<(
    ValueId,
    ScalarConstantValue,
    omega_optimization_core::ScalarConstantFactIdentity,
)> {
    fn merge(target: &mut ValidatorSccpValue, incoming: ValidatorSccpValue) -> bool {
        let next = match (*target, incoming) {
            (ValidatorSccpValue::Unknown, incoming) => incoming,
            (_, ValidatorSccpValue::Unknown) | (ValidatorSccpValue::Overdefined, _) => {
                return false;
            }
            (_, ValidatorSccpValue::Overdefined) => ValidatorSccpValue::Overdefined,
            (ValidatorSccpValue::Constant(current), ValidatorSccpValue::Constant(incoming))
                if current == incoming =>
            {
                return false;
            }
            (ValidatorSccpValue::Constant(_), ValidatorSccpValue::Constant(_)) => {
                ValidatorSccpValue::Overdefined
            }
        };
        if *target == next {
            false
        } else {
            *target = next;
            true
        }
    }

    let mut values = BTreeMap::<ValueId, ValidatorSccpValue>::new();
    for parameter in &function.parameters {
        values.insert(parameter.value, ValidatorSccpValue::Overdefined);
    }
    for block in &function.blocks {
        for parameter in &block.parameters {
            values.insert(parameter.value, ValidatorSccpValue::Unknown);
        }
        for definition in block.nodes.iter().flat_map(|node| &node.definitions) {
            values.insert(definition.value, ValidatorSccpValue::Overdefined);
        }
    }
    let support_blocks = function
        .blocks
        .iter()
        .flat_map(|block| {
            block.nodes.iter().flat_map(move |node| {
                node.provenance
                    .iter()
                    .filter_map(move |source| match source {
                        PsiProvenance::Operation(operation) => Some((*operation, block.id)),
                        PsiProvenance::Edge(_) => None,
                    })
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut literal_rows = Vec::new();
    let mut literal_support = BTreeMap::new();
    for fact in &function.facts {
        let (value, constant, support) = match fact {
            OptimizationFact::BooleanConstant {
                value,
                constant,
                support,
            } => (*value, ScalarConstantValue::Boolean(*constant), *support),
            OptimizationFact::IntegerConstant {
                value,
                constant,
                support,
            } => (*value, ScalarConstantValue::Integer(*constant), *support),
            OptimizationFact::OperationObligationReference { .. } => continue,
        };
        let block = support_blocks.get(&support).copied();
        literal_rows.push((value, constant, block));
        literal_support.insert(value, support);
        values.insert(
            value,
            if block.is_some() {
                ValidatorSccpValue::Unknown
            } else {
                ValidatorSccpValue::Constant(constant)
            },
        );
    }

    let mut reachable = BTreeSet::from([function.entry]);
    let mut feasible_edges = BTreeSet::<EdgeId>::new();
    loop {
        let mut changed = false;
        for block in &function.blocks {
            if !reachable.contains(&block.id) {
                continue;
            }
            for (value, constant, site) in &literal_rows {
                if *site == Some(block.id)
                    && matches!(values.get(value), Some(ValidatorSccpValue::Unknown))
                {
                    values.insert(*value, ValidatorSccpValue::Constant(*constant));
                    changed = true;
                }
            }
            let Some(node) = block.nodes.last() else {
                continue;
            };
            let operation_successors = validator_scalar_operation_successors(&node.operation);
            let successors = match &node.operation {
                omega_abstract_operations::AbstractOperation::Jump { .. } => {
                    operation_successors.iter().collect::<Vec<_>>()
                }
                omega_abstract_operations::AbstractOperation::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => match values.get(condition) {
                    Some(ValidatorSccpValue::Constant(ScalarConstantValue::Boolean(value))) => {
                        let selected = if *value {
                            when_true.psi_edge
                        } else {
                            when_false.psi_edge
                        };
                        operation_successors
                            .iter()
                            .filter(|successor| successor.psi_edge == selected)
                            .collect()
                    }
                    Some(ValidatorSccpValue::Overdefined) => {
                        operation_successors.iter().collect::<Vec<_>>()
                    }
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            for successor in successors {
                changed |= feasible_edges.insert(successor.psi_edge);
                changed |= reachable.insert(successor.target);
                for binding in &successor.bindings {
                    let incoming = values
                        .get(&binding.argument)
                        .copied()
                        .unwrap_or(ValidatorSccpValue::Overdefined);
                    let target = values
                        .entry(binding.parameter)
                        .or_insert(ValidatorSccpValue::Unknown);
                    changed |= merge(target, incoming);
                }
            }
        }
        if !changed {
            break;
        }
    }

    let snapshot = validator_sccp_snapshot(function, &values, &reachable, &feasible_edges);
    values
        .into_iter()
        .filter_map(|(value, state)| {
            let ValidatorSccpValue::Constant(constant) = state else {
                return None;
            };
            let definition = scalar_value_definition(function, value)?;
            let identity = literal_support
                .get(&value)
                .and_then(|support| {
                    literal_scalar_constant_fact_identity(
                        input,
                        function.machine,
                        definition,
                        constant,
                        *support,
                    )
                })
                .or_else(|| {
                    derived_sccp_scalar_constant_fact_identity(
                        input,
                        function.machine,
                        definition,
                        constant,
                        &snapshot,
                    )
                })?;
            Some((value, constant, identity))
        })
        .collect()
}

pub(crate) fn validator_scalar_operation_successors(
    operation: &omega_abstract_operations::AbstractOperation,
) -> Vec<OptimizationEdge> {
    use omega_abstract_operations::AbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            trivial_affine_discards,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![omega_optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(*psi_edge),
                units: 1,
            }],
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|successor| OptimizationEdge {
                psi_edge: successor.psi_edge,
                target: successor.target,
                bindings: successor.bindings.clone(),
                trivial_affine_discards: successor.trivial_affine_discards.clone(),
                provenance: vec![PsiProvenance::Edge(successor.psi_edge)],
                fuel: vec![omega_optimization_unit::FuelSettlement {
                    site: PsiProvenance::Edge(successor.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn validator_sccp_snapshot(
    function: &PsiOptimizationFunction,
    values: &BTreeMap<ValueId, ValidatorSccpValue>,
    reachable: &BTreeSet<BlockId>,
    feasible_edges: &BTreeSet<EdgeId>,
) -> SccpMachineSnapshot {
    use omega_abstract_operations::AbstractOperation as O;
    let mut blocks = function
        .blocks
        .iter()
        .map(|block| SccpBlockRow {
            block: block.id,
            executable: reachable.contains(&block.id),
        })
        .collect::<Vec<_>>();
    blocks.sort_by_key(|row| row.block);
    let mut edges = function
        .blocks
        .iter()
        .flat_map(|block| {
            let reachable_source = reachable.contains(&block.id);
            block.nodes.last().into_iter().flat_map(move |node| {
                validator_scalar_operation_successors(&node.operation)
                    .into_iter()
                    .map(move |successor| {
                        let state = if feasible_edges.contains(&successor.psi_edge) {
                            SccpEdgeState::Executable
                        } else if !reachable_source {
                            SccpEdgeState::Inexecutable
                        } else if let O::Conditional { condition, .. } = &node.operation {
                            match values.get(condition) {
                                Some(ValidatorSccpValue::Constant(
                                    ScalarConstantValue::Boolean(_),
                                )) => SccpEdgeState::Inexecutable,
                                _ => SccpEdgeState::Unknown,
                            }
                        } else {
                            SccpEdgeState::Inexecutable
                        };
                        SccpEdgeRow {
                            source: block.id,
                            edge: successor.psi_edge,
                            target: successor.target,
                            state,
                        }
                    })
            })
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|row| (row.source, row.edge));
    let mut snapshot_values = values
        .iter()
        .filter_map(|(value, state)| {
            let definition = scalar_value_definition(function, *value)?;
            Some(SccpValueRow {
                definition,
                state: match state {
                    ValidatorSccpValue::Unknown => SccpValueState::Unknown,
                    ValidatorSccpValue::Constant(ScalarConstantValue::Boolean(value)) => {
                        SccpValueState::Boolean(*value)
                    }
                    ValidatorSccpValue::Constant(ScalarConstantValue::Integer(value)) => {
                        SccpValueState::Integer(*value)
                    }
                    ValidatorSccpValue::Overdefined => SccpValueState::Overdefined,
                },
            })
        })
        .collect::<Vec<_>>();
    snapshot_values.sort_by_key(|row| row.definition.value);
    SccpMachineSnapshot {
        blocks,
        edges,
        values: snapshot_values,
    }
}

pub(crate) fn scalar_value_definition(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<ValueDefinition> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.definitions),
        )
        .copied()
        .find(|definition| definition.value == value)
}

pub(crate) fn validator_integer_value_type(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<psi_core::IntegerType> {
    scalar_value_definition(function, value).and_then(|definition| match definition.scalar_type {
        ScalarType::Integer(integer) => Some(integer),
        ScalarType::Boolean => None,
    })
}
