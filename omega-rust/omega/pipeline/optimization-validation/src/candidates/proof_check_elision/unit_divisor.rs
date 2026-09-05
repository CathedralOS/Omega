//! Exact remainder-by-unit laws and candidate acceptance.

use super::identity_classification::*;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IndependentRemainderUnitConstant {
    psi_operation: OperationId,
    obligation: semantic_vocabulary::ObligationId,
    result: ValueId,
    scalar_type: IntegerType,
    left: ValueId,
    right: ValueId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndependentRemainderUnitDivisor {
    PositiveOne,
    SignedNegativeOne,
}

impl IndependentRemainderUnitDivisor {
    fn value(self, scalar_type: IntegerType) -> Option<IntegerValue> {
        match self {
            Self::PositiveOne => Some(independent_integer_one(scalar_type)),
            Self::SignedNegativeOne if scalar_type.sign() == IntegerSign::Signed => {
                Some(IntegerValue::Signed(-1))
            }
            Self::SignedNegativeOne => None,
        }
    }
}

pub(crate) fn independent_remainder_unit_constant(
    operation: &O,
    divisor: IndependentRemainderUnitDivisor,
) -> Option<IndependentRemainderUnitConstant> {
    let (psi_operation, obligation, result, scalar_type, left, right) = match operation {
        O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } if scalar_type.carrier() == IntegerCarrier::Fixed && left != right => (
            *psi_operation,
            *obligation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        _ => return None,
    };
    divisor.value(scalar_type)?;
    Some(IndependentRemainderUnitConstant {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left,
        right,
    })
}

/// Independently validate the defined integer laws `x % 1 = 0` for exact,
/// wrapping, and saturating fixed-width integers. The right operand must be a
/// direct typed literal, and the authored operation must retain its exact
/// verifier-accepted obligation even though the literal also establishes that
/// the divisor is nonzero.
pub fn validate_proof_certified_integer_remainder_by_one_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_proof_certified_integer_remainder_by_unit_candidate(
        input,
        candidate,
        IndependentRemainderUnitDivisor::PositiveOne,
        b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1",
        b"omega.validator.live-proof-certified-integer-remainder-by-one-elimination.v1",
    )
}

/// Independently validate the defined signed integer laws `x % -1 = 0` for
/// exact, wrapping, and saturating fixed-width integers. The right operand must
/// be a direct typed literal, and the authored operation must retain its exact
/// verifier-accepted obligation. For exact arithmetic that accepted obligation
/// proves the otherwise exceptional signed-minimum input is unreachable.
pub fn validate_proof_certified_signed_integer_remainder_by_negative_one_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_proof_certified_integer_remainder_by_unit_candidate(
        input,
        candidate,
        IndependentRemainderUnitDivisor::SignedNegativeOne,
        b"omega.psi-rule.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
        b"omega.validator.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
    )
}

pub(crate) fn validate_proof_certified_integer_remainder_by_unit_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    divisor: IndependentRemainderUnitDivisor,
    rule_domain: &[u8],
    validator_domain: &[u8],
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let expected_rule = OptimizationRuleIdentity::from_canonical_bytes(rule_domain);
    if candidate.rule() != expected_rule
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
        || !candidate.substitutions().is_empty()
        || candidate.predicted_cost_delta() != -1
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location)
        || candidate.affected_blocks() != [patch.location.block]
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
    let node = block
        .nodes
        .get(
            usize::try_from(patch.location.node)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
        )
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let shape = independent_remainder_unit_constant(&node.operation, divisor)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if patch.source_operation != shape.psi_operation
        || patch.result != shape.result
        || patch.scalar_type != shape.scalar_type
        || patch.constant != independent_integer_zero(shape.scalar_type)
        || node.definitions
            != [ValueDefinition {
                value: shape.result,
                scalar_type: ScalarType::Integer(shape.scalar_type),
                site: ValueDefinitionSite::Node {
                    block: patch.location.block,
                    node: patch.location.node,
                },
            }]
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }

    let right_definition = scalar_value_definition(function, shape.right)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if right_definition.scalar_type != ScalarType::Integer(shape.scalar_type) {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let ValueDefinitionSite::Node {
        block: literal_block,
        node: literal_node,
    } = right_definition.site
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let literal = function
        .blocks
        .iter()
        .find(|block| block.id == literal_block)
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
    let expected_one = divisor
        .value(shape.scalar_type)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if literal_result != shape.right
        || literal_type != ScalarType::Integer(shape.scalar_type)
        || literal_value != expected_one
        || !shape.scalar_type.admits(expected_one)
        || !function.facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::IntegerConstant { value, constant, support }
                if *value == shape.right
                    && *constant == expected_one
                    && *support == literal_support)
        })
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let expected_constant_fact = literal_scalar_constant_fact_identity(
        input.identity,
        function.machine,
        right_definition,
        ScalarConstantValue::Integer(expected_one),
        literal_support,
    )
    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let expected_obligation_fact = independently_accepted_operation_fact(
        input,
        function,
        shape.psi_operation,
        shape.obligation,
    )
    .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
    let Some((constant_fact, obligation_fact)) =
        candidate.proof_certified_scalar_identity_witness()
    else {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    };
    if constant_fact != expected_constant_fact {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    if obligation_fact != expected_obligation_fact {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }

    let input_observation = observation_at(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !input_observation.events.is_empty()
        || !input_observation.successors.is_empty()
        || !input_observation.ownership.is_empty()
        || input_observation.crash != ObservationKnowledge::No
        || input_observation.suspension != ObservationKnowledge::No
    {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !input_live.live_in.contains(&shape.left)
        || !input_live.live_in.contains(&shape.right)
        || !input_live.live_out.contains(&shape.result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let site = PsiRealizationSite::Node(patch.location);
    let expected_provenance = [ProvenanceRewrite {
        input: site,
        disposition: ProvenanceDisposition::RealizedAt(site),
        sources: node.provenance.clone(),
        fuel: node.fuel.clone(),
    }];
    if candidate.provenance() != expected_provenance {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let accepted_catalog = input.accepted_obligation_facts.clone();
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate source function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate source block exists");
    let output_node = &mut output_block.nodes[patch.location.node as usize];
    output_node.operation = O::IntegerConstant {
        psi_operation: shape.psi_operation,
        result: shape.result,
        scalar_type: ScalarType::Integer(shape.scalar_type),
        value: patch.constant,
    };
    output_node.definitions = vec![ValueDefinition {
        value: shape.result,
        scalar_type: ScalarType::Integer(shape.scalar_type),
        site: ValueDefinitionSite::Node {
            block: patch.location.block,
            node: patch.location.node,
        },
    }];
    output_node.uses.clear();
    output_node.successors.clear();
    output_node.ownership.clear();
    output_function.facts = reconstruct_fact_index(output_function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    if output.accepted_obligation_facts != accepted_catalog {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
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
        validator: OptimizationValidatorIdentity::from_canonical_bytes(validator_domain),
        provenance: expected_provenance.into(),
    })
}
