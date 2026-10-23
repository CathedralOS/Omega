//! The one acceptance protocol the five proof-certified integer constant laws
//! share: reconstruct the law's exact symbolic shape from the authored
//! operation, require the candidate's patch, provenance and witness to be that
//! shape and that evidence, replay the closed scalar observation and the live
//! boundary, install the typed constant, and publish the law's validator
//! identity.
//!
//! A law states only what is genuinely its own: the exact rule and validator
//! identities, the analysis roster its proposal needs, the recognizer that
//! reconstructs its shape, and the check its candidate's evidence must
//! satisfy. The rest is the same operation for every law, so it is written
//! once here rather than restated per law; the two family copies it replaces
//! carried that identical acceptance sequence — contract, patch shape,
//! observation, live boundary, provenance, installation and custody guard —
//! and differed only in their law-owned evidence step, their analysis roster,
//! and the point at which the evidence was checked.

use crate::{
    OptimizationUnitValidationError, ValidatedPsiRewrite, reconstruct_closed_scalar_node_boundary,
};
use abstract_operations::{AbstractOperation, operations::IntegerArithmeticOperands};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationSafetyClass,
    OptimizationValidatorIdentity,
};
use optimization_unit::{
    ObservationKnowledge, PsiOptimizationFunction, PsiOptimizationUnit, PsiRealizationSite,
    PsiRewriteCandidate, PsiRewritePatch, ValueDefinition, ValueDefinitionSite,
};
use semantic_vocabulary::{
    IntegerType, IntegerValue, ObligationId, OperationId, ScalarType, ValueId,
};

/// Independently reconstructed closed scalar interface of one law's authored
/// operation: the operation, its discharged obligation, the defined result,
/// the operand pair, and the constants the law's proof and its replacement
/// name. Canonical ordering is by `ValueId`.
#[derive(Debug, Clone, Copy)]
pub(super) struct IndependentIntegerConstantLawShape {
    pub(super) psi_operation: OperationId,
    pub(super) obligation: ObligationId,
    pub(super) result: ValueId,
    pub(super) scalar_type: IntegerType,
    /// The dividend or left operand.
    pub(super) left: ValueId,
    /// The divisor or right operand.
    pub(super) right: ValueId,
    /// The typed constant the law replaces the whole operation with.
    pub(super) replacement: IntegerValue,
    /// The value of the divisor literal the law's evidence materializes, for
    /// the laws whose proof is that literal rather than the authored
    /// operation's own accepted obligation.
    pub(super) divisor_literal: Option<IntegerValue>,
}

/// Reconstruct one law's symbolic shape from an authored operation.
pub(super) type LawShapeRecogniser =
    fn(&AbstractOperation) -> Option<IndependentIntegerConstantLawShape>;

/// Check that the candidate carries exactly the evidence this law's proof
/// requires.
pub(super) type LawEvidenceCheck = fn(
    &PsiOptimizationUnit,
    &PsiOptimizationFunction,
    &IndependentIntegerConstantLawShape,
    &PsiRewriteCandidate,
) -> Result<(), OptimizationUnitValidationError>;

/// One proof-certified integer constant law and the exact protocol inputs that
/// are its own.
#[derive(Debug, Clone, Copy)]
pub(super) struct IndependentIntegerConstantLaw {
    /// The exact producer rule identity this law replays.
    pub(super) rule_domain: &'static [u8],
    /// The validator identity one accepted rewrite of this law publishes.
    pub(super) validator_domain: &'static [u8],
    /// The analyses the proposal needs; the literal-divisor laws also read the
    /// constant facts their evidence names.
    pub(super) required_analyses: AnalysisSet,
    pub(super) recognise: LawShapeRecogniser,
    pub(super) verify_evidence: LawEvidenceCheck,
}

/// The accepted obligation fact of the law's authored operation, reconstructed
/// from the input's retained custody. Both candidate witness shapes compare
/// against this one reconstruction instead of trusting a carried field.
pub(super) fn accepted_law_obligation(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    shape: &IndependentIntegerConstantLawShape,
) -> Result<optimization_core::AcceptedObligationFactIdentity, OptimizationUnitValidationError> {
    crate::candidates::global_value_numbering::independently_accepted_operation_fact(
        input,
        function,
        shape.psi_operation,
        shape.obligation,
    )
    .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
}

/// Independently validate and materialize one law's constant replacement: the
/// operation stays at its authored site and becomes the law's typed constant
/// only if the exact law, the exact evidence and the same closed scalar
/// observation and live boundary survive.
pub(super) fn validate_integer_constant_law_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    law: &IndependentIntegerConstantLaw,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    crate::candidates::contract::validate(
        input,
        candidate,
        law.rule_domain,
        law.required_analyses,
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        OptimizationSafetyClass::ProofCertified,
    )?;
    if !candidate.substitutions().is_empty() || candidate.predicted_cost_delta() != -1 {
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
    let function = crate::candidates::candidate_function(input, patch.location.machine)?;
    let block = crate::candidates::candidate_block(function, patch.location.block)?;
    let node = crate::candidates::candidate_node(block, patch.location.node)?;
    let shape = (law.recognise)(&node.operation)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if patch.source_operation != shape.psi_operation
        || patch.result != shape.result
        || patch.scalar_type != shape.scalar_type
        || patch.constant != shape.replacement
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
    (law.verify_evidence)(input, function, &shape, candidate)?;
    let input_observation =
        crate::candidates::sparse_conditional_constant_propagation::observation_at(
            input,
            patch.location,
        )
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
    let expected_provenance = [crate::candidates::provenance_realized_at(
        site,
        site,
        &node.provenance,
        &node.fuel,
    )];
    if candidate.provenance() != expected_provenance {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let output = crate::candidates::constant_installation::install_evaluated_constant(
        input,
        patch.location,
        AbstractOperation::IntegerConstant {
            psi_operation: shape.psi_operation,
            result: shape.result,
            scalar_type: ScalarType::Integer(shape.scalar_type),
            value: patch.constant,
        },
        shape.result,
        ScalarType::Integer(shape.scalar_type),
        &input_observation,
        &input_live,
    )?;
    // The law consumed its obligation; a replacement may not add to or drop
    // the retained accepted-obligation catalog.
    if output.accepted_obligation_facts != input.accepted_obligation_facts {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(law.validator_domain),
        provenance: expected_provenance.into(),
    })
}

/// The half of a law's shape every constant law shares: the operation's own
/// row, its discharged obligation, the result, the operand pair, and the
/// constant the law's replacement names.
pub(super) fn constant_law_shape(
    operands: &IntegerArithmeticOperands,
    replacement: IntegerValue,
    divisor_literal: Option<IntegerValue>,
) -> Option<IndependentIntegerConstantLawShape> {
    Some(IndependentIntegerConstantLawShape {
        psi_operation: operands.psi_operation,
        obligation: operands.arithmetic.obligation()?,
        result: operands.result,
        scalar_type: operands.scalar_type,
        left: operands.left,
        right: operands.right,
        replacement,
        divisor_literal,
    })
}
