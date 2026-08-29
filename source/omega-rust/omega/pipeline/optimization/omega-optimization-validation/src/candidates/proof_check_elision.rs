//! Proof-certified scalar rewrite validation.

use super::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct IndependentProofCertifiedScalarIdentity {
    source_operation: OperationId,
    obligation: psi_core::ObligationId,
    result: ValueId,
    replacement: ValueId,
    identity_operand: ValueId,
    result_type: IntegerType,
    identity_type: IntegerType,
    identity_constant: IntegerValue,
}

pub(crate) fn independent_proof_certified_scalar_identity(
    operation: &O,
    identity: ProofCertifiedScalarIdentityKind,
) -> Option<IndependentProofCertifiedScalarIdentity> {
    let row = match (operation, identity) {
        (
            O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *right,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *right,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type,
                count_type,
                value,
                count,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *count,
            *value_type,
            *count_type,
            independent_integer_zero(*count_type),
        ),
        (
            O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                count_type,
                value,
                count,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *count,
            *value_type,
            *count_type,
            independent_integer_zero(*count_type),
        ),
        (
            O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_one(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                right,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *right,
            *right,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                ..
            },
            ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *left,
            *left,
            *scalar_type,
            *scalar_type,
            independent_integer_zero(*scalar_type),
        ),
        (
            O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type,
                value,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *value,
            *value_type,
            *value_type,
            independent_integer_zero(*value_type),
        ),
        (
            O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                value,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
        ) => (
            *psi_operation,
            *obligation,
            *result,
            *value,
            *value,
            *value_type,
            *value_type,
            independent_integer_zero(*value_type),
        ),
        (
            O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                value,
                ..
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue,
        ) if value_type.carrier() == IntegerCarrier::Fixed
            && value_type.sign() == IntegerSign::Signed =>
        {
            (
                *psi_operation,
                *obligation,
                *result,
                *value,
                *value,
                *value_type,
                *value_type,
                IntegerValue::Signed(-1),
            )
        }
        _ => return None,
    };
    Some(IndependentProofCertifiedScalarIdentity {
        source_operation: row.0,
        obligation: row.1,
        result: row.2,
        replacement: row.3,
        identity_operand: row.4,
        result_type: row.5,
        identity_type: row.6,
        identity_constant: row.7,
    })
}

pub(crate) fn independent_integer_zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}

pub(crate) fn independent_integer_one(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    }
}

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
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::ScalarConstants)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::ProofCertified
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

impl std::fmt::Display for OptimizationUnitValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Psi optimization unit: {self:?}")
    }
}

impl std::error::Error for OptimizationUnitValidationError {}

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
    obligation: psi_core::ObligationId,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IndependentRemainderUnitConstant {
    psi_operation: OperationId,
    obligation: psi_core::ObligationId,
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
