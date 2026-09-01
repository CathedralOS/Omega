//! Binary integer constant-evaluation fixtures by exact operation identity.

use super::*;

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
    pub(crate) fn proof_certified(self) -> bool {
        matches!(
            self,
            Self::ExactAdd
                | Self::ExactSubtract
                | Self::ExactMultiply
                | Self::ExactDivide
                | Self::ExactRemainder
                | Self::WrappingDivide
                | Self::WrappingRemainder
                | Self::SaturatingDivide
                | Self::SaturatingRemainder
                | Self::ExactShiftLeft
                | Self::ExactShiftRight
        )
    }

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
        match self {
            Self::ExactAdd => O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::ExactSubtract => O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::ExactMultiply => O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::WrappingAdd => O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::WrappingSubtract => O::WrappingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::WrappingMultiply => O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::SaturatingAdd => O::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::SaturatingSubtract => O::SaturatingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::SaturatingMultiply => O::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::ExactDivide => O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::ExactRemainder => O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::WrappingDivide => O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::WrappingRemainder => O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::SaturatingDivide => O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::SaturatingRemainder => O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            Self::ExactShiftLeft => O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type: scalar_type,
                count_type,
                value: left,
                count: right,
            },
            Self::ExactShiftRight => O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type: scalar_type,
                count_type,
                value: left,
                count: right,
            },
            Self::WrappingShiftLeft => O::WrappingIntegerShiftLeft {
                psi_operation,
                result,
                value_type: scalar_type,
                count_type,
                value: left,
                count: right,
            },
            Self::WrappingShiftRight => O::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type: scalar_type,
                count_type,
                value: left,
                count: right,
            },
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
            structural_types: Vec::new(),
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
