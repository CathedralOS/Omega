use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::TotalScalarIdentityKind;
use psi_core::{IntegerSign, IntegerType, IntegerValue, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::rules::passes) struct TotalScalarIdentityShape {
    pub source_operation: OperationId,
    pub result: ValueId,
    pub replacement: ValueId,
    pub identity_operand: ValueId,
    pub scalar_type: IntegerType,
    pub identity_operand_type: IntegerType,
    pub identity: TotalScalarIdentityKind,
    pub expected_identity_value: IntegerValue,
}

/// Return the exact semantic rows in canonical tie order.
///
/// For commutative operations the left-identity row precedes the right row.
/// A candidate whose two operands are both neutral therefore has one stable,
/// reviewable disposition instead of depending on incidental fact order.
pub(in crate::rules::passes) fn wrapping_neutral_identity_shapes(
    operation: &O,
) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => vec![
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *right,
                identity_operand: *left,
                scalar_type: *scalar_type,
                identity_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerAddZeroLeft,
                expected_identity_value: integer_value(*scalar_type, 0),
            },
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                identity_operand: *right,
                scalar_type: *scalar_type,
                identity_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerAddZeroRight,
                expected_identity_value: integer_value(*scalar_type, 0),
            },
        ],
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => vec![TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *left,
            identity_operand: *right,
            scalar_type: *scalar_type,
            identity_operand_type: *scalar_type,
            identity: TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight,
            expected_identity_value: integer_value(*scalar_type, 0),
        }],
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => vec![
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *right,
                identity_operand: *left,
                scalar_type: *scalar_type,
                identity_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft,
                expected_identity_value: integer_value(*scalar_type, 1),
            },
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                identity_operand: *right,
                scalar_type: *scalar_type,
                identity_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight,
                expected_identity_value: integer_value(*scalar_type, 1),
            },
        ],
        _ => Vec::new(),
    }
}

/// Return the two wrapping shift laws in canonical left/right-shift order.
pub(in crate::rules::passes) fn wrapping_shift_zero_count_identity_shapes(
    operation: &O,
) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => vec![TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *value,
            identity_operand: *count,
            scalar_type: *value_type,
            identity_operand_type: *count_type,
            identity: TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount,
            expected_identity_value: integer_value(*count_type, 0),
        }],
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => vec![TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *value,
            identity_operand: *count,
            scalar_type: *value_type,
            identity_operand_type: *count_type,
            identity: TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount,
            expected_identity_value: integer_value(*count_type, 0),
        }],
        _ => Vec::new(),
    }
}

const fn integer_value(scalar_type: IntegerType, value: u128) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value as i128),
        IntegerSign::Unsigned => IntegerValue::Unsigned(value),
    }
}
