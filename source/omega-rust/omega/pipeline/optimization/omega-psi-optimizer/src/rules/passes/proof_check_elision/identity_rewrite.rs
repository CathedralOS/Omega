//! Shared construction for proof-certified scalar identity rewrites.

use super::*;

pub(super) fn propose_proof_certified_scalar_identities(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    shapes_for_operation: fn(&O) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let Some(AnalysisProduct::UseDefinition(use_definitions)) =
        analyses.get(AnalysisKind::UseDefinition)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::UseDefinition,
        ));
    };
    let Some(AnalysisProduct::EffectSummaries(effects)) =
        analyses.get(AnalysisKind::EffectSummaries)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::EffectSummaries,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let shapes = shapes_for_operation(&node.operation);
                if shapes.is_empty() {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                let effect = effects.nodes.iter().find(|row| {
                    row.machine == function.machine
                        && row.block == block.id
                        && row.node == node_index
                });
                if effect.is_none_or(|row| {
                    row.revision != unit.identity
                        || row.class != crate::EffectClass::PureScalar
                        || row.observable != crate::EffectKnowledge::No
                        || row.structural_state != crate::EffectKnowledge::No
                        || row.crash != crate::EffectKnowledge::No
                        || row.suspension != crate::EffectKnowledge::No
                }) {
                    continue;
                }
                let Some((patch_shape, constant_fact)) =
                    shapes.into_iter().find_map(|(shape, expected)| {
                        let (actual, fact) = literal_integer_constant(
                            constants,
                            function.machine,
                            shape.identity_operand,
                        )?;
                        (actual == expected).then_some((shape, fact))
                    })
                else {
                    continue;
                };
                if !use_definitions.uses.iter().any(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == patch_shape.result
                }) {
                    continue;
                }
                let Ok(obligation_fact) =
                    accepted_obligation_fact(unit, function.machine, patch_shape.source_operation)
                else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                let Some((affected_blocks, provenance)) =
                    local_cse_accounting(function, location, patch_shape.result)
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_proof_certified_scalar_identity(
                        unit.identity,
                        contract,
                        affected_blocks,
                        provenance,
                        constant_fact,
                        obligation_fact,
                        -1,
                        ProofCertifiedScalarIdentityRewrite {
                            location,
                            source_operation: patch_shape.source_operation,
                            result: patch_shape.result,
                            replacement: patch_shape.replacement,
                            scalar_type: patch_shape.scalar_type,
                            identity: patch_shape.identity,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ProofCertifiedScalarIdentityShape {
    source_operation: OperationId,
    result: ValueId,
    replacement: ValueId,
    identity_operand: ValueId,
    scalar_type: IntegerType,
    identity: ProofCertifiedScalarIdentityKind,
}

pub(super) fn proof_certified_scalar_identity_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    match operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => vec![
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *right,
                    identity_operand: *left,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft,
                },
                integer_zero(*scalar_type),
            ),
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *left,
                    identity_operand: *right,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight,
                },
                integer_zero(*scalar_type),
            ),
        ],
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                identity_operand: *right,
                scalar_type: *scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight,
            },
            integer_zero(*scalar_type),
        )],
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => vec![
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *right,
                    identity_operand: *left,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
                },
                integer_one(*scalar_type),
            ),
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *left,
                    identity_operand: *right,
                    scalar_type: *scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight,
                },
                integer_one(*scalar_type),
            ),
        ],
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count,
            value,
            count_type,
            ..
        } => vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *value,
                identity_operand: *count,
                scalar_type: *value_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount,
            },
            integer_zero(*count_type),
        )],
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count,
            value,
            count_type,
            ..
        } => vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *value,
                identity_operand: *count,
                scalar_type: *value_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount,
            },
            integer_zero(*count_type),
        )],
        _ => Vec::new(),
    }
}

pub(super) fn proof_certified_integer_divide_by_one_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let (source_operation, result, scalar_type, left, right, identity) = match operation {
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight,
        ),
        _ => return Vec::new(),
    };
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            identity_operand: right,
            scalar_type,
            identity,
        },
        integer_one(scalar_type),
    )]
}

pub(super) fn proof_certified_exact_integer_multiply_by_zero_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let O::ExactIntegerMultiply {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = operation
    else {
        return Vec::new();
    };
    vec![
        (
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                identity_operand: *left,
                scalar_type: *scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
            },
            integer_zero(*scalar_type),
        ),
        (
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *right,
                identity_operand: *right,
                scalar_type: *scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
            },
            integer_zero(*scalar_type),
        ),
    ]
}

pub(super) fn proof_certified_integer_zero_dividend_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let (source_operation, result, scalar_type, left, identity) = match operation {
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft,
        ),
        _ => return Vec::new(),
    };
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            identity_operand: left,
            scalar_type,
            identity,
        },
        integer_zero(scalar_type),
    )]
}

pub(super) fn proof_certified_exact_integer_zero_value_shift_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let (source_operation, result, scalar_type, value, identity) = match operation {
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            value,
            ..
        } => (
            *psi_operation,
            *result,
            *value_type,
            *value,
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            value,
            ..
        } => (
            *psi_operation,
            *result,
            *value_type,
            *value,
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
        ),
        _ => return Vec::new(),
    };
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: value,
            identity_operand: value,
            scalar_type,
            identity,
        },
        integer_zero(scalar_type),
    )]
}

pub(in crate::rules::passes) fn integer_zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        psi_core::IntegerSign::Signed => IntegerValue::Signed(0),
        psi_core::IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}

pub(in crate::rules::passes) fn integer_one(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        psi_core::IntegerSign::Signed => IntegerValue::Signed(1),
        psi_core::IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    }
}
