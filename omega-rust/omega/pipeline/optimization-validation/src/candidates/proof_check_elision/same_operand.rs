//! Same-operand exact integer laws and candidate acceptance.

use super::identity_classification::*;
use super::*;

/// Independently reconstructed scalar interface of one closed node region.
/// Canonical ordering is by `ValueId`; block-parameter bindings remain uses of
/// the predecessor terminator and therefore participate naturally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProofCertifiedSameOperandIntegerConstantLaw {
    ExactSubtractZero,
    SelfRemainderZero,
    SelfDivideOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IndependentSameOperandIntegerConstant {
    psi_operation: OperationId,
    obligation: semantic_vocabulary::ObligationId,
    result: ValueId,
    scalar_type: IntegerType,
    operand: ValueId,
}

pub(crate) fn independent_same_operand_integer_constant(
    operation: &O,
    law: ProofCertifiedSameOperandIntegerConstantLaw,
) -> Option<IndependentSameOperandIntegerConstant> {
    let (psi_operation, obligation, result, scalar_type, left, right) = match (law, operation) {
        (
            ProofCertifiedSameOperandIntegerConstantLaw::ExactSubtractZero,
            O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        (
            ProofCertifiedSameOperandIntegerConstantLaw::SelfRemainderZero,
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
            },
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        (
            ProofCertifiedSameOperandIntegerConstantLaw::SelfDivideOne,
            O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        ) if scalar_type.carrier() == IntegerCarrier::Fixed
            && !(scalar_type.sign() == IntegerSign::Signed && scalar_type.bits() == 1) =>
        {
            (
                *psi_operation,
                *obligation,
                *result,
                *scalar_type,
                *left,
                *right,
            )
        }
        _ => return None,
    };
    (left == right).then_some(IndependentSameOperandIntegerConstant {
        psi_operation,
        obligation,
        result,
        scalar_type,
        operand: left,
    })
}

/// Independently validate and materialize the exact symbolic law `x - x = 0`.
pub fn validate_proof_certified_exact_integer_self_subtract_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_proof_certified_same_operand_integer_constant_candidate(
        input,
        candidate,
        ProofCertifiedSameOperandIntegerConstantLaw::ExactSubtractZero,
        b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1",
        b"omega.validator.live-proof-certified-exact-integer-self-subtract-elimination.v1",
    )
}

/// Independently validate the defined remainder laws `x % x = 0` for exact,
/// wrapping, and saturating fixed-width integers. The accepted obligation is
/// required because it is the capability proving the authored divisor is
/// legal; no operand constant or range fact is inferred.
pub fn validate_proof_certified_integer_self_remainder_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_proof_certified_same_operand_integer_constant_candidate(
        input,
        candidate,
        ProofCertifiedSameOperandIntegerConstantLaw::SelfRemainderZero,
        b"omega.psi-rule.live-proof-certified-integer-self-remainder-elimination.v1",
        b"omega.validator.live-proof-certified-integer-self-remainder-elimination.v1",
    )
}

/// Independently validate the defined division laws `x / x = 1` for exact,
/// wrapping, and saturating fixed-width integers. The accepted obligation is
/// the capability proving that the authored divisor is nonzero (and that the
/// signed overflow case is absent). Signed one-bit integers are excluded
/// because typed positive one is not representable.
pub fn validate_proof_certified_integer_self_divide_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_proof_certified_same_operand_integer_constant_candidate(
        input,
        candidate,
        ProofCertifiedSameOperandIntegerConstantLaw::SelfDivideOne,
        b"omega.psi-rule.live-proof-certified-integer-self-divide-elimination.v1",
        b"omega.validator.live-proof-certified-integer-self-divide-elimination.v1",
    )
}

/// Validate only the shared in-place constant custody. The operation-law
/// selector remains validation-local and closed, so adding one producer rule
/// cannot broaden another rule's accepted policy vocabulary.
pub(crate) fn validate_proof_certified_same_operand_integer_constant_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    law: ProofCertifiedSameOperandIntegerConstantLaw,
    expected_rule_domain: &[u8],
    validator_domain: &[u8],
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let expected_rule = OptimizationRuleIdentity::from_canonical_bytes(expected_rule_domain);
    if candidate.rule() != expected_rule
        || candidate.required_analyses()
            != AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries])
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
    let shape = independent_same_operand_integer_constant(&node.operation, law)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if patch.source_operation != shape.psi_operation
        || patch.result != shape.result
        || patch.scalar_type != shape.scalar_type
        || patch.constant
            != match law {
                ProofCertifiedSameOperandIntegerConstantLaw::ExactSubtractZero
                | ProofCertifiedSameOperandIntegerConstantLaw::SelfRemainderZero => {
                    independent_integer_zero(shape.scalar_type)
                }
                ProofCertifiedSameOperandIntegerConstantLaw::SelfDivideOne => {
                    independent_integer_one(shape.scalar_type)
                }
            }
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
    if !input_live.live_in.contains(&shape.operand) || !input_live.live_out.contains(&shape.result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let expected_fact = independently_accepted_operation_fact(
        input,
        function,
        shape.psi_operation,
        shape.obligation,
    )
    .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
    if candidate.accepted_obligation_witness() != Some(expected_fact) {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
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
