//! Binary integer constant-evaluation fixtures by exact operation identity.

use super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractResult, BlockId, EdgeId, FuelScheduleIdentity, IntegerType,
    IntegerValue, MachineId, O, ObligationId, OperationId, PsiOptimizationUnit, ScalarType,
    SemanticFingerprint, TerminalPsiIdentity, ValueId, VocabularyMarker, id,
    reconstruct_psi_optimization_unit_seed, with_synthetic_accepted_obligations,
};
use abstract_operations::{
    IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy, IntegerShiftDirection,
    IntegerShiftPolicy,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum BinaryConstantFixtureKind {
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
    ExactShiftLeft,
    ExactShiftRight,
    WrappingShiftLeft,
    WrappingShiftRight,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

impl BinaryConstantFixtureKind {
    fn operation(
        self,
        psi_operation: OperationId,
        obligation: ObligationId,
        result: ValueId,
        scalar_type: IntegerType,
        count_type: IntegerType,
        left: ValueId,
        right: ValueId,
    ) -> AbstractOperation {
        use IntegerArithmeticOperator as Operator;
        use IntegerOverflowPolicy as Policy;
        let arithmetic = |policy, operator| O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic: IntegerArithmetic::new(
                policy,
                operator,
                IntegerArithmetic::requires_obligation(policy, operator).then_some(obligation),
            )
            .expect("admitted integer arithmetic"),
            left,
            right,
        };
        let shift = |policy, direction| O::IntegerShift {
            psi_operation,
            result,
            value_type: scalar_type,
            count_type,
            policy,
            direction,
            value: left,
            count: right,
        };
        let exact_shift = IntegerShiftPolicy::Exact { obligation };
        match self {
            Self::ExactAdd => arithmetic(Policy::Exact, Operator::Add),
            Self::ExactSubtract => arithmetic(Policy::Exact, Operator::Subtract),
            Self::ExactMultiply => arithmetic(Policy::Exact, Operator::Multiply),
            Self::WrappingAdd => arithmetic(Policy::Wrapping, Operator::Add),
            Self::WrappingSubtract => arithmetic(Policy::Wrapping, Operator::Subtract),
            Self::WrappingMultiply => arithmetic(Policy::Wrapping, Operator::Multiply),
            Self::SaturatingAdd => arithmetic(Policy::Saturating, Operator::Add),
            Self::SaturatingSubtract => arithmetic(Policy::Saturating, Operator::Subtract),
            Self::SaturatingMultiply => arithmetic(Policy::Saturating, Operator::Multiply),
            Self::ExactDivide => arithmetic(Policy::Exact, Operator::Divide),
            Self::ExactRemainder => arithmetic(Policy::Exact, Operator::Remainder),
            Self::WrappingDivide => arithmetic(Policy::Wrapping, Operator::Divide),
            Self::WrappingRemainder => arithmetic(Policy::Wrapping, Operator::Remainder),
            Self::SaturatingDivide => arithmetic(Policy::Saturating, Operator::Divide),
            Self::SaturatingRemainder => arithmetic(Policy::Saturating, Operator::Remainder),
            Self::ExactShiftLeft => shift(exact_shift, IntegerShiftDirection::Left),
            Self::ExactShiftRight => shift(exact_shift, IntegerShiftDirection::Right),
            Self::WrappingShiftLeft => {
                shift(IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Left)
            }
            Self::WrappingShiftRight => {
                shift(IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Right)
            }
            Self::BitwiseAnd => O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::BitwiseOr => O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::BitwiseXor => O::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
        }
    }
}

pub(crate) fn binary_constant_unit(
    kind: BinaryConstantFixtureKind,
    scalar_type: IntegerType,
    count_type: IntegerType,
    left_constant: IntegerValue,
    right_constant: IntegerValue,
) -> PsiOptimizationUnit {
    let machine = id(341, MachineId::new);
    let block = id(342, BlockId::new);
    let left = id(343, ValueId::new);
    let right = id(344, ValueId::new);
    let result = id(345, ValueId::new);
    let operation = kind.operation(
        id(348, OperationId::new),
        id(349, ObligationId::new),
        result,
        scalar_type,
        count_type,
        left,
        right,
    );
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([35; 32]),
            },
            entry: machine,
            structural_types: Vec::new().into(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type: ScalarType::Integer(scalar_type),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(346, OperationId::new),
                        result: left,
                        scalar_type: ScalarType::Integer(scalar_type),
                        value: left_constant,
                    },
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(347, OperationId::new),
                        result: right,
                        scalar_type: ScalarType::Integer(count_type),
                        value: right_constant,
                    },
                    operation,
                    AbstractOperation::Return {
                        psi_edge: id(350, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(scalar_type),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    with_synthetic_accepted_obligations(unit)
}
