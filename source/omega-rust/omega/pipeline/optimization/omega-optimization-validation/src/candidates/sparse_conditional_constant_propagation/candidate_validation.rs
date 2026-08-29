//! Scalar, boolean, range, and observation candidate acceptance.

use super::integer_evaluation::*;
use super::snapshot_reconstruction::validator_integer_value_type;
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
        | PsiRewritePatch::EliminateTotalScalarIdentity(_)
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
