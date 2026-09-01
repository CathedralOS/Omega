//! Proof-certified scalar identity candidate acceptance.

use super::identity_classification::*;
use super::*;

/// Independently remove one live proof-certified integer identity.
/// Accepted proof and literal evidence are reconstructed from immutable input
/// custody; the declared operation is deleted rather than reclassified.
pub fn validate_proof_certified_scalar_identity_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let exact_identity_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-integer-identity-elimination.v1",
    );
    let divide_by_one_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-integer-divide-by-one-elimination.v1",
    );
    let multiply_by_zero_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1",
    );
    let zero_dividend_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-integer-zero-dividend-elimination.v1",
    );
    let zero_value_shift_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-exact-integer-zero-value-shift-elimination.v1",
    );
    let negative_one_shift_right_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1",
    );
    if ![
        exact_identity_rule,
        divide_by_one_rule,
        multiply_by_zero_rule,
        zero_dividend_rule,
        zero_value_shift_rule,
        negative_one_shift_right_rule,
    ]
    .contains(&candidate.rule())
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
        || candidate.predicted_cost_delta() != -1
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let validator = match candidate.rule() {
        rule if rule == exact_identity_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight
                    | ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight
                    | ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight
                    | ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount
                    | ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount
            ) =>
        {
            b"omega.validator.live-proof-certified-integer-identity-elimination.v1".as_slice()
        }
        rule if rule == divide_by_one_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight
                    | ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight
                    | ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight
            ) =>
        {
            b"omega.validator.live-proof-certified-integer-divide-by-one-elimination.v1".as_slice()
        }
        rule if rule == multiply_by_zero_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight
            ) =>
        {
            b"omega.validator.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1"
                .as_slice()
        }
        rule if rule == zero_dividend_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft
                    | ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft
                    | ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft
                    | ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft
                    | ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft
                    | ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft
            ) =>
        {
            b"omega.validator.live-proof-certified-integer-zero-dividend-elimination.v1".as_slice()
        }
        rule if rule == zero_value_shift_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue
                    | ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue
            ) =>
        {
            b"omega.validator.live-proof-certified-exact-integer-zero-value-shift-elimination.v1"
                .as_slice()
        }
        rule if rule == negative_one_shift_right_rule
            && matches!(
                patch.identity,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue
            ) =>
        {
            b"omega.validator.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1"
                .as_slice()
        }
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    if candidate.node_decision_point() != Some(patch.location)
        || candidate.substitutions()
            != [ScalarSubstitution {
                from: patch.result,
                to: patch.replacement,
                scalar_type: ScalarType::Integer(patch.scalar_type),
            }]
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
    let node_index = usize::try_from(patch.location.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let shape = independent_proof_certified_scalar_identity(&node.operation, patch.identity)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if shape.source_operation != patch.source_operation
        || shape.result != patch.result
        || shape.replacement != patch.replacement
        || shape.result_type != patch.scalar_type
        || node.definitions
            != [ValueDefinition {
                value: patch.result,
                scalar_type: ScalarType::Integer(patch.scalar_type),
                site: ValueDefinitionSite::Node {
                    block: block.id,
                    node: patch.location.node,
                },
            }]
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
        || block.nodes.get(node_index + 1).is_none()
        || !function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.uses)
            .any(|use_site| use_site.value == patch.result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if scalar_value_definition(function, shape.replacement)
        .is_none_or(|definition| definition.scalar_type != ScalarType::Integer(shape.result_type))
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let identity_definition = scalar_value_definition(function, shape.identity_operand)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    if identity_definition.scalar_type != ScalarType::Integer(shape.identity_type) {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let ValueDefinitionSite::Node {
        block: literal_block,
        node: literal_node,
    } = identity_definition.site
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
    if literal_result != shape.identity_operand
        || literal_type != ScalarType::Integer(shape.identity_type)
        || literal_value != shape.identity_constant
        || !function.facts.iter().any(|fact| {
            matches!(fact, OptimizationFact::IntegerConstant { value, constant, support }
                if *value == shape.identity_operand
                    && *constant == shape.identity_constant
                    && *support == literal_support)
        })
    {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    let expected_constant_fact = literal_scalar_constant_fact_identity(
        input.identity,
        function.machine,
        identity_definition,
        ScalarConstantValue::Integer(shape.identity_constant),
        literal_support,
    )
    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let Some((constant_fact, obligation_fact)) =
        candidate.proof_certified_scalar_identity_witness()
    else {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    };
    if constant_fact != expected_constant_fact {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    }
    if !function.facts.iter().any(|fact| {
        matches!(fact, OptimizationFact::OperationObligationReference { obligation, support }
            if *obligation == shape.obligation && *support == shape.source_operation)
    }) || !input.accepted_obligation_facts.iter().any(|fact| {
        fact.identity == obligation_fact
            && fact.machine == function.machine
            && fact.operation == shape.source_operation
            && fact.obligation == shape.obligation
    }) {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
    let receiver = &block.nodes[node_index + 1];
    if receiver
        .provenance
        .iter()
        .any(|source| node.provenance.contains(source))
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_proof_certified_scalar_identity_accounting(function, patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate block exists");
    let removed = output_block.nodes.remove(node_index);
    let receiver = output_block
        .nodes
        .get_mut(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&removed.provenance);
    receiver.fuel.extend_from_slice(&removed.fuel);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            rewrite_scalar_value_uses(&mut node.operation, patch.result, patch.replacement);
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = effect
                .checked_add(1)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    if output.accepted_obligation_facts != input.accepted_obligation_facts {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
    let output_function = output
        .functions
        .iter()
        .find(|output_function| output_function.machine == function.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(validator),
        provenance: accepted_provenance,
    })
}
