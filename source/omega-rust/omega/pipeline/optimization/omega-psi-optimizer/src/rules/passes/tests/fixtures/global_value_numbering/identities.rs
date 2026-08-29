//! Typed fixture for obligation-free wrapping identity rows.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WrappingNeutralOperation {
    Add,
    Subtract,
    Multiply,
}

pub(crate) fn wrapping_neutral_identity_unit(
    operation: WrappingNeutralOperation,
    literal_value: IntegerValue,
    literal_left: bool,
    both_operands_literal: bool,
) -> PsiOptimizationUnit {
    wrapping_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        operation,
        literal_value,
        literal_left,
        both_operands_literal,
        true,
    )
}

pub(crate) fn wrapping_neutral_identity_unit_with_type_and_liveness(
    integer: IntegerType,
    operation: WrappingNeutralOperation,
    literal_value: IntegerValue,
    literal_left: bool,
    both_operands_literal: bool,
    result_is_live: bool,
) -> PsiOptimizationUnit {
    let machine = id(1_901, MachineId::new);
    let block = id(1_902, BlockId::new);
    let other = id(1_903, ValueId::new);
    let literal = id(1_904, ValueId::new);
    let result = id(1_905, ValueId::new);
    let literal_operation = id(1_906, OperationId::new);
    let identity_operation = id(1_907, OperationId::new);
    let scalar_type = ScalarType::Integer(integer);
    let (left, right) = if both_operands_literal {
        (literal, literal)
    } else if literal_left {
        (literal, other)
    } else {
        (other, literal)
    };
    let operation = match operation {
        WrappingNeutralOperation::Add => O::WrappingIntegerAdd {
            psi_operation: identity_operation,
            result,
            scalar_type: integer,
            left,
            right,
        },
        WrappingNeutralOperation::Subtract => O::WrappingIntegerSubtract {
            psi_operation: identity_operation,
            result,
            scalar_type: integer,
            left,
            right,
        },
        WrappingNeutralOperation::Multiply => O::WrappingIntegerMultiply {
            psi_operation: identity_operation,
            result,
            scalar_type: integer,
            left,
            right,
        },
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([49; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![AbstractParameter {
                    value: other,
                    scalar_type,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: if result_is_live { result } else { other },
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    O::IntegerConstant {
                        psi_operation: literal_operation,
                        result: literal,
                        scalar_type,
                        value: literal_value,
                    },
                    operation,
                    O::Return {
                        psi_edge: id(1_908, EdgeId::new),
                        result: if result_is_live { result } else { other },
                        value: if result_is_live { result } else { other },
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}
