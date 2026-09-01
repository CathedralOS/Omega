//! Fresh typed fixtures for Boolean-result constant evaluation.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BooleanFixtureKind {
    Not,
    Equal,
}

pub(crate) fn boolean_constant_unit(
    kind: BooleanFixtureKind,
    left_value: bool,
    right_value: bool,
) -> PsiOptimizationUnit {
    let machine = id(341, MachineId::new);
    let block = id(342, BlockId::new);
    let left = id(343, ValueId::new);
    let right = id(344, ValueId::new);
    let result = id(345, ValueId::new);
    let operation = match kind {
        BooleanFixtureKind::Not => AbstractOperation::BooleanNot {
            psi_operation: id(348, OperationId::new),
            result,
            operand: left,
        },
        BooleanFixtureKind::Equal => AbstractOperation::BooleanEqual {
            psi_operation: id(348, OperationId::new),
            result,
            left,
            right,
        },
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([16; 32]),
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
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::BooleanConstant {
                        psi_operation: id(346, OperationId::new),
                        result: left,
                        value: left_value,
                    },
                    AbstractOperation::BooleanConstant {
                        psi_operation: id(347, OperationId::new),
                        result: right,
                        value: right_value,
                    },
                    operation,
                    AbstractOperation::Return {
                        psi_edge: id(349, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Boolean,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn boolean_unit(equal: bool) -> PsiOptimizationUnit {
    let kind = if equal {
        BooleanFixtureKind::Equal
    } else {
        BooleanFixtureKind::Not
    };
    boolean_constant_unit(kind, true, false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComparisonFixtureKind {
    Equal,
    LessThan,
    LessOrEqual,
}

pub(crate) fn integer_comparison_constant_unit(
    kind: ComparisonFixtureKind,
    scalar_type: IntegerType,
    left_value: IntegerValue,
    right_value: IntegerValue,
) -> PsiOptimizationUnit {
    let machine = id(351, MachineId::new);
    let block = id(352, BlockId::new);
    let left = id(353, ValueId::new);
    let right = id(354, ValueId::new);
    let result = id(355, ValueId::new);
    let operation = match kind {
        ComparisonFixtureKind::Equal => AbstractOperation::IntegerEqual {
            psi_operation: id(358, OperationId::new),
            result,
            left,
            right,
        },
        ComparisonFixtureKind::LessThan => AbstractOperation::IntegerLessThan {
            psi_operation: id(358, OperationId::new),
            result,
            left,
            right,
        },
        ComparisonFixtureKind::LessOrEqual => AbstractOperation::IntegerLessOrEqual {
            psi_operation: id(358, OperationId::new),
            result,
            left,
            right,
        },
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([17; 32]),
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
                    scalar_type: ScalarType::Boolean,
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
                        psi_operation: id(356, OperationId::new),
                        result: left,
                        scalar_type: ScalarType::Integer(scalar_type),
                        value: left_value,
                    },
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(357, OperationId::new),
                        result: right,
                        scalar_type: ScalarType::Integer(scalar_type),
                        value: right_value,
                    },
                    operation,
                    AbstractOperation::Return {
                        psi_edge: id(359, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Boolean,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}
