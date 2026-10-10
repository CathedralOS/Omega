//! Shared mechanics of the obligation-free total-scalar identity rules: the
//! shape a rule's law partition selects, the contract they share, and the
//! traversal that joins a selected shape to its literal and effect evidence.

use abstract_operations::{AbstractOperation, IntegerArithmeticOperator, IntegerOverflowPolicy};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityKind,
    TotalScalarIdentityRewrite,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};

use crate::rules::GLOBAL_VALUE_NUMBERING_PASS_NAME;
use crate::rules::global_value_numbering::exact_pure_scalar_effect;
use crate::rules::node_elision::node_elision_accounting;
use crate::{AnalysisProduct, RuleAnalysisView, RuleProposalError};

pub(super) fn propose_total_scalar_identities(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    classify: fn(&AbstractOperation) -> Vec<TotalScalarIdentityShape>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;

    let mut candidates = Vec::new();
    for function in &unit.functions {
        let value_types = super::function_value_types(function);
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let shapes = classify(&node.operation);
                if shapes.is_empty() {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                if !exact_pure_scalar_effect(unit, effects, function.machine, block.id, node_index)
                {
                    continue;
                }
                let Some((shape, constant_fact)) = shapes.into_iter().find_map(|shape| {
                    if value_types.get(&shape.law_operand)
                        != Some(&ScalarType::Integer(shape.law_operand_type))
                    {
                        return None;
                    }
                    let (actual, fact) =
                        constants.literal_integer_constant(function.machine, shape.law_operand)?;
                    (actual == shape.expected_law_value).then_some((shape, fact))
                }) else {
                    continue;
                };
                if !use_definitions.uses.iter().any(|(machine, use_site)| {
                    *machine == function.machine && use_site.value == shape.result
                }) {
                    continue;
                }
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                let Some((affected_blocks, provenance)) =
                    node_elision_accounting(function, location, shape.result)
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_total_scalar_identity(
                        unit.identity,
                        contract,
                        affected_blocks,
                        provenance,
                        constant_fact,
                        -1,
                        TotalScalarIdentityRewrite {
                            location,
                            source_operation: shape.source_operation,
                            result: shape.result,
                            replacement: shape.replacement,
                            scalar_type: shape.scalar_type,
                            identity: shape.identity,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TotalScalarIdentityShape {
    pub source_operation: OperationId,
    pub result: ValueId,
    pub replacement: ValueId,
    pub law_operand: ValueId,
    pub scalar_type: IntegerType,
    pub law_operand_type: IntegerType,
    pub identity: TotalScalarIdentityKind,
    pub expected_law_value: IntegerValue,
}

const REQUIRED_ANALYSES: [AnalysisKind; 3] = [
    AnalysisKind::ScalarConstants,
    AnalysisKind::UseDefinition,
    AnalysisKind::EffectSummaries,
];

const INVALIDATED_ANALYSES: [AnalysisKind; 2] =
    [AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries];

pub(super) fn exact_total_scalar_identity(identity: &[u8]) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(identity),
        OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
        1,
        AnalysisSet::new(REQUIRED_ANALYSES),
        AnalysisInvalidationSet::new(INVALIDATED_ANALYSES),
        OptimizationSafetyClass::ExactOperationSemantics,
    )
    .expect("built-in rule has nonzero version")
}

pub(super) fn all_ones(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(-1),
        IntegerSign::Unsigned => scalar_type.maximum_value(),
    }
}

/// Shapes for a multiply-zero annihilation rule under one overflow policy:
/// the left-zero then right-zero laws so ties stay deterministic.
pub(super) fn multiply_zero_annihilation_shapes(
    operation: &AbstractOperation,
    policy: IntegerOverflowPolicy,
    left_zero: TotalScalarIdentityKind,
    right_zero: TotalScalarIdentityKind,
) -> Vec<TotalScalarIdentityShape> {
    let AbstractOperation::IntegerArithmetic {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        arithmetic,
        ..
    } = operation
    else {
        return Vec::new();
    };
    if !arithmetic.is(policy, IntegerArithmeticOperator::Multiply) {
        return Vec::new();
    }
    vec![
        TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *left,
            law_operand: *left,
            scalar_type: *scalar_type,
            law_operand_type: *scalar_type,
            identity: left_zero,
            expected_law_value: scalar_type.zero(),
        },
        TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *right,
            law_operand: *right,
            scalar_type: *scalar_type,
            law_operand_type: *scalar_type,
            identity: right_zero,
            expected_law_value: scalar_type.zero(),
        },
    ]
}

/// The neutral-arithmetic law-kind table one overflow policy classifies
/// under.
pub(super) struct NeutralArithmeticKinds {
    pub add_zero_left: TotalScalarIdentityKind,
    pub add_zero_right: TotalScalarIdentityKind,
    pub subtract_zero_right: TotalScalarIdentityKind,
    pub multiply_one_left: TotalScalarIdentityKind,
    pub multiply_one_right: TotalScalarIdentityKind,
}

/// Shapes for the neutral-arithmetic rules under one overflow policy:
/// add-zero, subtract-zero-right and multiply-one laws in canonical
/// left-literal/right-literal tie order.
pub(super) fn neutral_arithmetic_shapes(
    operation: &AbstractOperation,
    policy: IntegerOverflowPolicy,
    kinds: NeutralArithmeticKinds,
) -> Vec<TotalScalarIdentityShape> {
    let Some(operands) = operation.integer_arithmetic() else {
        return Vec::new();
    };
    if operands.arithmetic.policy() != policy {
        return Vec::new();
    }
    let zero = operands.scalar_type.zero();
    let one = operands.scalar_type.one();
    // (replacement, law operand, identity, expected law value)
    let laws = match operands.arithmetic.operator() {
        IntegerArithmeticOperator::Add => vec![
            (operands.right, operands.left, kinds.add_zero_left, zero),
            (operands.left, operands.right, kinds.add_zero_right, zero),
        ],
        IntegerArithmeticOperator::Subtract => vec![(
            operands.left,
            operands.right,
            kinds.subtract_zero_right,
            zero,
        )],
        IntegerArithmeticOperator::Multiply => vec![
            (operands.right, operands.left, kinds.multiply_one_left, one),
            (operands.left, operands.right, kinds.multiply_one_right, one),
        ],
        IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder => Vec::new(),
    };
    laws.into_iter()
        .map(
            |(replacement, law_operand, identity, expected_law_value)| TotalScalarIdentityShape {
                source_operation: operands.psi_operation,
                result: operands.result,
                replacement,
                law_operand,
                scalar_type: operands.scalar_type,
                law_operand_type: operands.scalar_type,
                identity,
                expected_law_value,
            },
        )
        .collect()
}
